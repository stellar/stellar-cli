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

type GetLedgerEntriesRequest struct {
	Keys []string `json:"keys"`
}

type LedgerEntryResponse struct {
	XDR                string `json:"xdr"`
	LastModifiedLedger int64  `json:"lastModifiedLedgerSeq,string"`
}

type GetLedgerEntriesResponse struct {
	Entries      []*LedgerEntryResponse `json:"entries"`
	LatestLedger int64                  `json:"latestLedger,string"`
}

// NewGetLedgerEntriesHandler returns a JSON RPC handler to retrieve the specified ledger entries from Stellar Core.
func NewGetLedgerEntriesHandler(logger *log.Entry, ledgerEntryReader db.LedgerEntryReader) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request GetLedgerEntriesRequest) (GetLedgerEntriesResponse, error) {
		var ledgerKeys []xdr.LedgerKey
		for i, requestKey := range request.Keys {
			var ledgerKey xdr.LedgerKey
			if err := xdr.SafeUnmarshalBase64(requestKey, &ledgerKey); err != nil {
				logger.WithError(err).WithField("request", request).
					Infof("could not unmarshal requestKey %s at index %d from getLedgerEntries request", requestKey, i)
				return GetLedgerEntriesResponse{}, &jrpc2.Error{
					Code:    code.InvalidParams,
					Message: fmt.Sprintf("cannot unmarshal key value %s at index %d", requestKey, i),
				}
			}
			ledgerKeys = append(ledgerKeys, ledgerKey)
		}

		tx, err := ledgerEntryReader.NewTx(ctx)
		if err != nil {
			return GetLedgerEntriesResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not create read transaction",
			}
		}
		defer func() {
			_ = tx.Done()
		}()

		latestLedger, err := tx.GetLatestLedgerSequence()
		if err != nil {
			return GetLedgerEntriesResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not get latest ledger",
			}
		}

		var ledgerEntryResponses []*LedgerEntryResponse
		for i, ledgerKey := range ledgerKeys {
			present, ledgerEntry, err := tx.GetLedgerEntry(ledgerKey)
			if err != nil {
				logger.WithError(err).WithField("request", request).
					Infof("could not obtain ledger entry %s at index %d from storage", ledgerKey, i)
				return GetLedgerEntriesResponse{}, &jrpc2.Error{
					Code:    code.InternalError,
					Message: fmt.Sprintf("could not obtain ledger entry %v at index %d from storage", ledgerKey, i),
				}
			}

			if !present {
				ledgerEntryResponses = append(ledgerEntryResponses, nil)
				continue
			}

			ledgerXDR, err := xdr.MarshalBase64(ledgerEntry.Data)
			if err != nil {
				logger.WithError(err).WithField("request", request).
					Infof("could not serialize ledger entry data for ledger %v at index %d", ledgerEntry, i)
				return GetLedgerEntriesResponse{}, &jrpc2.Error{
					Code:    code.InternalError,
					Message: fmt.Sprintf("could not serialize ledger entry data for ledger %v at index %d", ledgerEntry, i),
				}
			}

			ledgerEntryResponses = append(ledgerEntryResponses, &LedgerEntryResponse{
				XDR:                ledgerXDR,
				LastModifiedLedger: int64(ledgerEntry.LastModifiedLedgerSeq),
			})
		}

		response := GetLedgerEntriesResponse{
			Entries:      ledgerEntryResponses,
			LatestLedger: int64(latestLedger),
		}
		return response, nil
	})
}
