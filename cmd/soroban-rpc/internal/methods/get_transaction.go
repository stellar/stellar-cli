package methods

import (
	"context"
	"encoding/base64"
	"encoding/hex"
	"fmt"

	"github.com/creachadair/jrpc2"
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
	LatestLedger uint32 `json:"latestLedger"`
	// LatestLedgerCloseTime is the unix timestamp of when the latest ledger was closed.
	LatestLedgerCloseTime int64 `json:"latestLedgerCloseTime,string"`
	// LatestLedger is the oldest ledger stored in Soroban-RPC.
	OldestLedger uint32 `json:"oldestLedger"`
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
	Ledger uint32 `json:"ledger,omitempty"`
	// LedgerCloseTime is the unix timestamp of when the transaction was included in the ledger.
	LedgerCloseTime int64 `json:"createdAt,string,omitempty"`

	// DiagnosticEventsXDR is present only if Status is equal to TransactionFailed.
	// DiagnosticEventsXDR is a base64-encoded slice of xdr.DiagnosticEvent
	DiagnosticEventsXDR []string `json:"diagnosticEventsXdr,omitempty"`
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
			Code:    jrpc2.InvalidParams,
			Message: fmt.Sprintf("unexpected hash length (%d)", len(request.Hash)),
		}
	}

	var txHash xdr.Hash
	_, err := hex.Decode(txHash[:], []byte(request.Hash))
	if err != nil {
		return GetTransactionResponse{}, &jrpc2.Error{
			Code:    jrpc2.InvalidParams,
			Message: fmt.Sprintf("incorrect hash: %v", err),
		}
	}

	tx, found, storeRange := getter.GetTransaction(txHash)
	response := GetTransactionResponse{
		LatestLedger:          storeRange.LastLedger.Sequence,
		LatestLedgerCloseTime: storeRange.LastLedger.CloseTime,
		OldestLedger:          storeRange.FirstLedger.Sequence,
		OldestLedgerCloseTime: storeRange.FirstLedger.CloseTime,
	}
	if !found {
		response.Status = TransactionStatusNotFound
		return response, nil
	}

	response.ApplicationOrder = tx.ApplicationOrder
	response.FeeBump = tx.FeeBump
	response.Ledger = tx.Ledger.Sequence
	response.LedgerCloseTime = tx.Ledger.CloseTime

	response.ResultXdr = base64.StdEncoding.EncodeToString(tx.Result)
	response.EnvelopeXdr = base64.StdEncoding.EncodeToString(tx.Envelope)
	response.ResultMetaXdr = base64.StdEncoding.EncodeToString(tx.Meta)
	response.DiagnosticEventsXDR = base64EncodeSlice(tx.Events)

	if tx.Successful {
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
