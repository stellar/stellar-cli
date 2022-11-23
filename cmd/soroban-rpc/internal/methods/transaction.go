package methods

import (
	"context"
	"encoding/hex"
	"fmt"
	"net/http"
	"sync"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"

	"github.com/stellar/go/clients/horizonclient"
	"github.com/stellar/go/network"
	"github.com/stellar/go/protocols/horizon"
	"github.com/stellar/go/xdr"
)

const (
	TransactionSuccess = "success"
	TransactionPending = "pending"
	TransactionError   = "error"
)

type SendTransactionRequest struct {
	Transaction string `json:"transaction"`
}

type GetTransactionStatusRequest struct {
	Hash string `json:"hash"`
}

type SCVal struct {
	XDR string `json:"xdr"`
}

type TransactionResponseError struct {
	Code    string                 `json:"code"`
	Message string                 `json:"message"`
	Data    map[string]interface{} `json:"data"`
}

type TransactionStatusResponse struct {
	ID      string  `json:"id"`
	Status  string  `json:"status"`
	Results []SCVal `json:"results,omitempty"`
	// Error will be nil unless Status is equal to "error"
	Error *TransactionResponseError `json:"error,omitempty"`
}

type SendTransactionResponse struct {
	ID     string `json:"id"`
	Status string `json:"status"`
	// Error will be nil unless Status is equal to "error"
	Error *TransactionResponseError `json:"error"`
}

type transactionResult struct {
	timestamp time.Time
	pending   bool
	err       *TransactionResponseError
}

type horizonRequest struct {
	txHash         string
	transactionXDR string
}

type TransactionProxy struct {
	lock       sync.RWMutex
	results    map[string]transactionResult
	client     *horizonclient.Client
	passphrase string
	queue      chan horizonRequest
	workers    int
	ttl        time.Duration
	cancel     context.CancelFunc
	wg         sync.WaitGroup
}

func NewTransactionProxy(
	client *horizonclient.Client,
	workers, queueSize int,
	networkPassphrase string,
	ttl time.Duration,
) *TransactionProxy {
	if workers > queueSize {
		queueSize = workers
	}
	return &TransactionProxy{
		results:    map[string]transactionResult{},
		client:     client,
		passphrase: networkPassphrase,
		queue:      make(chan horizonRequest, queueSize),
		workers:    workers,
		ttl:        ttl,
	}
}

func (p *TransactionProxy) Start(ctx context.Context) {
	ctx, p.cancel = context.WithCancel(ctx)
	p.wg.Add(p.workers)
	for i := 0; i < p.workers; i++ {
		go p.startWorker(ctx)
	}
}

func (p *TransactionProxy) Close() {
	// signal the worker go routines to abort
	p.cancel()
	// wait until the worker go routines are done
	p.wg.Wait()
}

func (p *TransactionProxy) SendTransaction(ctx context.Context, request SendTransactionRequest) SendTransactionResponse {
	var envelope xdr.TransactionEnvelope
	err := xdr.SafeUnmarshalBase64(request.Transaction, &envelope)
	if err != nil {
		return SendTransactionResponse{
			Status: TransactionError,
			Error: &TransactionResponseError{
				Code:    "invalid_xdr",
				Message: fmt.Sprintf("cannot unmarshal transaction: %v", err),
			},
		}
	}

	var hash [32]byte
	hash, err = network.HashTransactionInEnvelope(envelope, p.passphrase)
	if err != nil {
		return SendTransactionResponse{
			Status: TransactionError,
			Error: &TransactionResponseError{
				Code:    "invalid_hash",
				Message: fmt.Sprintf("cannot hash transaction: %v", err),
			},
		}
	}
	txHash := hex.EncodeToString(hash[:])

	p.lock.Lock()
	defer func() {
		p.deleteExpiredEntries(time.Now())
		p.lock.Unlock()
	}()

	result, ok := p.results[txHash]
	// if pending or completed without any errors use
	// getTransactionStatus method with tx hash to obtain
	// response
	if result.pending || (ok && result.err == nil) {
		return SendTransactionResponse{
			ID:     txHash,
			Status: TransactionPending,
		}
	}

	p.results[txHash] = transactionResult{pending: true}
	select {
	case p.queue <- horizonRequest{txHash: txHash, transactionXDR: request.Transaction}:
		return SendTransactionResponse{
			ID:     txHash,
			Status: TransactionPending,
		}
	default:
		delete(p.results, txHash)
		return SendTransactionResponse{
			ID:     txHash,
			Status: TransactionError,
			Error: &TransactionResponseError{
				Code:    "full_queue",
				Message: "Transaction queue is full",
			},
		}
	}
}

func (p *TransactionProxy) setTxResult(txHash string, result transactionResult) {
	p.lock.Lock()
	defer p.lock.Unlock()
	p.results[txHash] = result
}

func (p *TransactionProxy) deletePendingEntry(txHash string) {
	p.lock.Lock()
	defer p.lock.Unlock()
	delete(p.results, txHash)
}

func (p *TransactionProxy) startWorker(ctx context.Context) {
	defer p.wg.Done()
	for {
		select {
		case <-ctx.Done():
			return
		case request := <-p.queue:
			_, err := p.client.SubmitTransactionXDR(request.transactionXDR)
			if err != nil {
				result := transactionResult{timestamp: time.Now()}
				if herr, ok := err.(*horizonclient.Error); ok {
					result.err = &TransactionResponseError{
						Code:    "tx_submission_failed",
						Message: "Transaction submission failed",
						Data:    herr.Problem.Extras,
					}
				} else {
					result.err = &TransactionResponseError{
						Code:    "http_error",
						Message: fmt.Sprintf("transaction submission failed: %v", err),
					}
				}
				p.setTxResult(request.txHash, result)
			} else {
				p.deletePendingEntry(request.txHash)
			}
		}
	}
}

func parseResults(tx horizon.Transaction) ([]SCVal, *TransactionResponseError) {
	var txResult xdr.TransactionResult
	if err := xdr.SafeUnmarshalBase64(tx.ResultXdr, &txResult); err != nil {
		return nil, &TransactionResponseError{
			Code:    "invalid_xdr",
			Message: fmt.Sprintf("cannot unmarshal transaction result: %v", err),
			Data: map[string]interface{}{
				"transaction": tx,
			},
		}
	}

	var scvals []SCVal
	opResults, ok := txResult.OperationResults()
	if !ok {
		return nil, &TransactionResponseError{
			Code:    "no_tx_results",
			Message: "Transaction succeeded but had no results",
			Data: map[string]interface{}{
				"transaction": tx,
			},
		}
	}

	for _, opResult := range opResults {
		result, ok := opResult.GetTr()
		if !ok {
			continue
		}
		invokeHostFunctionResult, ok := result.GetInvokeHostFunctionResult()
		if !ok {
			continue
		}
		scval, ok := invokeHostFunctionResult.GetSuccess()
		if !ok {
			continue
		}
		scvalB64, err := xdr.MarshalBase64(scval)
		if err != nil {
			return nil, &TransactionResponseError{
				Code:    "invalid_xdr",
				Message: fmt.Sprintf("cannot unmarshal scval: %v", err),
				Data: map[string]interface{}{
					"transaction": tx,
				},
			}
		}
		scvals = append(scvals, SCVal{XDR: scvalB64})
	}

	return scvals, nil
}

func (p *TransactionProxy) GetTransactionStatus(ctx context.Context, request GetTransactionStatusRequest) TransactionStatusResponse {
	tx, err := p.client.TransactionDetail(request.Hash)
	if err != nil {
		if herr, ok := err.(*horizonclient.Error); ok {
			if herr.Problem.Status != http.StatusNotFound {
				return TransactionStatusResponse{
					ID:     request.Hash,
					Status: TransactionError,
					Error: &TransactionResponseError{
						Code:    herr.Problem.Title,
						Message: herr.Problem.Detail,
						Data:    herr.Problem.Extras,
					},
				}
			}
		} else {
			return TransactionStatusResponse{
				ID:     request.Hash,
				Status: TransactionError,
				Error: &TransactionResponseError{
					Code:    "http_error",
					Message: fmt.Sprintf("transaction submission failed: %v", err),
				},
			}
		}
	} else {
		if !tx.Successful {
			return TransactionStatusResponse{
				ID:     request.Hash,
				Status: TransactionError,
				Error: &TransactionResponseError{
					Code:    "tx_failed",
					Message: "transaction included in ledger but failed",
					Data: map[string]interface{}{
						"transaction": tx,
					},
				},
			}
		}

		results, err := parseResults(tx)
		status := TransactionSuccess
		if err != nil {
			status = TransactionError
		}
		return TransactionStatusResponse{
			ID:      request.Hash,
			Status:  status,
			Results: results,
			Error:   err,
		}
	}

	// herr.Problem.Status == http.StatusNotFound
	// if the tx is not found perform the request
	p.lock.RLock()
	defer p.lock.RUnlock()
	result, ok := p.results[request.Hash]
	if !ok {
		return TransactionStatusResponse{
			ID:     request.Hash,
			Status: TransactionError,
			Error: &TransactionResponseError{
				Code:    "tx_not_found",
				Message: "transaction not found",
			},
		}
	}

	if result.pending {
		return TransactionStatusResponse{
			ID:     request.Hash,
			Status: TransactionPending,
		}
	}

	return TransactionStatusResponse{
		ID:     request.Hash,
		Status: TransactionError,
		Error:  result.err,
	}
}

// deleteExpiredEntries should only be called while the write lock is held
func (p *TransactionProxy) deleteExpiredEntries(now time.Time) {
	for key, val := range p.results {
		if !val.pending && now.Sub(val.timestamp) > p.ttl {
			delete(p.results, key)
		}
	}
}

// NewGetTransactionStatusHandler returns a get transaction json rpc handler
func NewGetTransactionStatusHandler(proxy *TransactionProxy) jrpc2.Handler {
	return handler.New(proxy.GetTransactionStatus)
}

// NewSendTransactionHandler returns a submit transaction json rpc handler
func NewSendTransactionHandler(proxy *TransactionProxy) jrpc2.Handler {
	return handler.New(proxy.SendTransaction)
}
