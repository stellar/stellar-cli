package methods

import (
	"context"
	"fmt"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"

	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

// Deprecated. Use GetLedgerEntriesRequest instead.
// TODO(https://github.com/stellar/soroban-tools/issues/374) remove after getLedgerEntries is deployed.
type GetLedgerEntryRequest struct {
	Key string `json:"key"`
}

// Deprecated. Use GetLedgerEntriesResponse instead.
// TODO(https://github.com/stellar/soroban-tools/issues/374) remove after getLedgerEntries is deployed.
type GetLedgerEntryResponse struct {
	XDR                string `json:"xdr"`
	LastModifiedLedger int64  `json:"lastModifiedLedgerSeq,string"`
	LatestLedger       int64  `json:"latestLedger,string"`
}

// NewGetLedgerEntryHandler returns a json rpc handler to retrieve the specified ledger entry from stellar core
// Deprecated. use NewGetLedgerEntriesHandler instead.
// TODO(https://github.com/stellar/soroban-tools/issues/374) remove after getLedgerEntries is deployed.
func NewGetLedgerEntryHandler(logger *log.Entry, ledgerEntryReader db.LedgerEntryReader) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request GetLedgerEntryRequest) (GetLedgerEntryResponse, error) {
		var key xdr.LedgerKey
		if err := xdr.SafeUnmarshalBase64(request.Key, &key); err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not unmarshal ledgerKey from getLedgerEntry request")
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InvalidParams,
				Message: "cannot unmarshal key value",
			}
		}

		tx, err := ledgerEntryReader.NewTx(ctx)
		if err != nil {
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not create read transaction",
			}
		}
		defer func() {
			_ = tx.Done()
		}()

		latestLedger, err := tx.GetLatestLedgerSequence()
		if err != nil {
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not get latest ledger",
			}
		}

		present, ledgerEntry, err := tx.GetLedgerEntry(key, false)
		if err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not obtain ledger entry from storage")
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not obtain ledger entry from storage",
			}
		}

		if !present {
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InvalidRequest,
				Message: fmt.Sprintf("not found (at ledger %d)", latestLedger),
			}
		}

		response := GetLedgerEntryResponse{
			LastModifiedLedger: int64(ledgerEntry.LastModifiedLedgerSeq),
			LatestLedger:       int64(latestLedger),
		}
		if response.XDR, err = xdr.MarshalBase64(ledgerEntry.Data); err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not serialize ledger entry data")
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not serialize ledger entry data",
			}
		}

		return response, nil
	})
}
