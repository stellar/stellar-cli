package methods

import (
	"context"
	"encoding/hex"
	"fmt"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

const (
	// TransactionStatusSuccess indicates the transaction was included in the ledger and
	// it was executed without errors.
	TransactionStatusSuccess = "SUCCESS"
	// TransactionStatusNotFound indicates the transaction was not found in Soroban-RPC's
	// transaction store.
	TransactionStatusNotFound = "NOT_FOUND"
	// TransactionStatusFailed indicates the transaction was included in the ledger and
	// it was executed with an error.
	TransactionStatusFailed = "FAILED"
)

// GetTransactionResponse is the response for the Soroban-RPC getTransaction() endpoint
type GetTransactionResponse struct {
	// Status is one of: TransactionSuccess, TransactionNotFound, or TransactionFailed.
	Status string `json:"status"`
	// LatestLedger is the latest ledger stored in Soroban-RPC.
	LatestLedger int64 `json:"latestLedger,string"`
	// LatestLedgerCloseTime is the unix timestamp of when the latest ledger was closed.
	LatestLedgerCloseTime int64 `json:"latestLedgerCloseTime,string"`
	// LatestLedger is the oldest ledger stored in Soroban-RPC.
	OldestLedger int64 `json:"oldestLedger,string"`
	// LatestLedgerCloseTime is the unix timestamp of when the oldest ledger was closed.
	OldestLedgerCloseTime int64 `json:"oldestLedgerCloseTime,string"`

	// The fields below are only present if Status is not TransactionNotFound.

	// ApplicationOrder is the index of the transaction among all the transactions
	// for that ledger.
	ApplicationOrder int32 `json:"applicationOrder,omitempty"`
	// FeeBump indicates whether the transaction is a feebump transaction
	FeeBump bool `json:"feeBump,omitempty"`
	// EnvelopeXdr is the TransactionEnvelope XDR value.
	EnvelopeXdr string `json:"envelopeXdr,omitempty"`
	// ResultXdr is the TransactionResult XDR value.
	ResultXdr string `json:"resultXdr,omitempty"`
	// ResultMetaXdr is the TransactionMeta XDR value.
	ResultMetaXdr string `json:"resultMetaXdr,omitempty"`

	// Ledger is the sequence of the ledger which included the transaction.
	Ledger int64 `json:"ledger,string,omitempty"`
	// LedgerCloseTime is the unix timestamp of when the transaction was included in the ledger.
	LedgerCloseTime int64 `json:"createdAt,string,omitempty"`
}

type GetTransactionRequest struct {
	Hash string `json:"hash"`
}

type transactionGetter interface {
	GetTransaction(hash xdr.Hash) (transactions.Transaction, bool, transactions.StoreRange)
}

func GetTransaction(getter transactionGetter, request GetTransactionRequest) (GetTransactionResponse, error) {
	// parse hash
	if hex.DecodedLen(len(request.Hash)) != len(xdr.Hash{}) {
		return GetTransactionResponse{}, &jrpc2.Error{
			Code:    code.InvalidParams,
			Message: fmt.Sprintf("unexpected hash length (%d)", len(request.Hash)),
		}
	}

	var txHash xdr.Hash
	_, err := hex.Decode(txHash[:], []byte(request.Hash))
	if err != nil {
		return GetTransactionResponse{}, &jrpc2.Error{
			Code:    code.InvalidParams,
			Message: fmt.Sprintf("incorrect hash: %v", err),
		}
	}

	tx, found, storeRange := getter.GetTransaction(txHash)
	response := GetTransactionResponse{
		LatestLedger:          int64(storeRange.LastLedger.Sequence),
		LatestLedgerCloseTime: storeRange.LastLedger.CloseTime,
		OldestLedger:          int64(storeRange.FirstLedger.Sequence),
		OldestLedgerCloseTime: storeRange.FirstLedger.CloseTime,
	}
	if !found {
		response.Status = TransactionStatusNotFound
		return response, nil
	}

	response.ApplicationOrder = tx.ApplicationOrder
	response.FeeBump = tx.FeeBump
	response.Ledger = int64(tx.Ledger.Sequence)
	response.LedgerCloseTime = tx.Ledger.CloseTime
	if response.ResultXdr, err = xdr.MarshalBase64(tx.Result); err != nil {
		return GetTransactionResponse{}, &jrpc2.Error{
			Code:    code.InternalError,
			Message: err.Error(),
		}
	}
	if response.EnvelopeXdr, err = xdr.MarshalBase64(tx.Envelope); err != nil {
		return GetTransactionResponse{}, &jrpc2.Error{
			Code:    code.InternalError,
			Message: err.Error(),
		}
	}
	if response.ResultMetaXdr, err = xdr.MarshalBase64(tx.Meta); err != nil {
		return GetTransactionResponse{}, &jrpc2.Error{
			Code:    code.InternalError,
			Message: err.Error(),
		}
	}
	if tx.Result.Successful() {
		response.Status = TransactionStatusSuccess
	} else {
		response.Status = TransactionStatusFailed
	}
	return response, nil
}

// NewGetTransactionHandler returns a get transaction json rpc handler
func NewGetTransactionHandler(getter transactionGetter) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request GetTransactionRequest) (GetTransactionResponse, error) {
		return GetTransaction(getter, request)
	})
}
