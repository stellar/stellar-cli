package methods

import (
	"context"
	"encoding/hex"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
	"github.com/stellar/go/gxdr"
	"github.com/stellar/go/network"
	proto "github.com/stellar/go/protocols/stellarcore"
	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

// SendTransactionResponse represents the transaction submission response returned Soroban-RPC
type SendTransactionResponse struct {
	// ErrorResultXDR is present only if Status is equal to proto.TXStatusError.
	// ErrorResultXDR is a TransactionResult xdr string which contains details on why
	// the transaction could not be accepted by stellar-core.
	ErrorResultXDR string `json:"errorResultXdr,omitempty"`
	// Status represents the status of the transaction submission returned by stellar-core.
	// Status can be one of: proto.TXStatusPending, proto.TXStatusDuplicate,
	// proto.TXStatusTryAgainLater, or proto.TXStatusError.
	Status string `json:"status"`
	// Hash is a hash of the transaction which can be used to look up whether
	// the transaction was included in the ledger.
	Hash string `json:"hash"`
	// LatestLedger is the latest ledger known to Soroban-RPC at the time it handled
	// the transaction submission request.
	LatestLedger uint32 `json:"latestLedger,string"`
	// LatestLedgerCloseTime is the unix timestamp of the close time of the latest ledger known to
	// Soroban-RPC at the time it handled the transaction submission request.
	LatestLedgerCloseTime int64 `json:"latestLedgerCloseTime,string"`
}

// SendTransactionRequest is the Soroban-RPC request to submit a transaction.
type SendTransactionRequest struct {
	// Transaction is the base64 encoded transaction envelope.
	Transaction string `json:"transaction"`
}

// LatestLedgerStore is a store which returns the latest ingested ledger.
type LatestLedgerStore interface {
	// GetLatestLedger returns the latest ingested ledger.
	GetLatestLedger() transactions.LedgerInfo
}

var invalidTransactionXdrError = &jrpc2.Error{
	Code:    jrpc2.InvalidParams,
	Message: "invalid_xdr",
}

// NewSendTransactionHandler returns a submit transaction json rpc handler
func NewSendTransactionHandler(daemon interfaces.Daemon, logger *log.Entry, store LatestLedgerStore, passphrase string) jrpc2.Handler {
	submitter := daemon.CoreClient()
	return handler.New(func(ctx context.Context, request SendTransactionRequest) (SendTransactionResponse, error) {
		if err := gxdr.ValidateTransactionEnvelope(request.Transaction, gxdr.DefaultMaxDepth); err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not validate send transaction envelope")
			return SendTransactionResponse{}, invalidTransactionXdrError
		}
		var envelope xdr.TransactionEnvelope
		err := xdr.SafeUnmarshalBase64(request.Transaction, &envelope)
		if err != nil {
			return SendTransactionResponse{}, invalidTransactionXdrError
		}

		var hash [32]byte
		hash, err = network.HashTransactionInEnvelope(envelope, passphrase)
		if err != nil {
			return SendTransactionResponse{}, &jrpc2.Error{
				Code:    jrpc2.InvalidParams,
				Message: "invalid_hash",
			}
		}
		txHash := hex.EncodeToString(hash[:])

		ledgerInfo := store.GetLatestLedger()
		resp, err := submitter.SubmitTransaction(ctx, request.Transaction)
		if err != nil {
			logger.WithError(err).
				WithField("tx", request.Transaction).Error("could not submit transaction")
			return SendTransactionResponse{}, &jrpc2.Error{
				Code:    jrpc2.InternalError,
				Message: "could not submit transaction to stellar-core",
			}
		}

		// interpret response
		if resp.IsException() {
			logger.WithField("exception", resp.Exception).
				WithField("tx", request.Transaction).Error("received exception from stellar core")
			return SendTransactionResponse{}, &jrpc2.Error{
				Code:    jrpc2.InternalError,
				Message: "received exception from stellar-core",
			}
		}

		switch resp.Status {
		case proto.TXStatusError:
			return SendTransactionResponse{
				ErrorResultXDR:        resp.Error,
				Status:                resp.Status,
				Hash:                  txHash,
				LatestLedger:          ledgerInfo.Sequence,
				LatestLedgerCloseTime: ledgerInfo.CloseTime,
			}, nil
		case proto.TXStatusPending, proto.TXStatusDuplicate, proto.TXStatusTryAgainLater:
			return SendTransactionResponse{
				Status:                resp.Status,
				Hash:                  txHash,
				LatestLedger:          ledgerInfo.Sequence,
				LatestLedgerCloseTime: ledgerInfo.CloseTime,
			}, nil
		default:
			logger.WithField("status", resp.Status).
				WithField("tx", request.Transaction).Error("Unrecognized stellar-core status response")
			return SendTransactionResponse{}, &jrpc2.Error{
				Code:    jrpc2.InternalError,
				Message: "invalid status from stellar-core",
			}
		}
	})
}
