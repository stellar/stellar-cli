package methods

import (
	"context"
	"fmt"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
	"github.com/stellar/go/gxdr"

	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

type GetLedgerEntriesRequest struct {
	Keys []string `json:"keys"`
}

type LedgerEntryResult struct {
	// Original request key matching this LedgerEntryResult.
	Key string `json:"key"`
	// Ledger entry data encoded in base 64.
	XDR string `json:"xdr"`
	// Last modified ledger for this entry.
	LastModifiedLedger int64 `json:"lastModifiedLedgerSeq,string"`
}

type GetLedgerEntriesResponse struct {
	// All found ledger entries.
	Entries []LedgerEntryResult `json:"entries"`
	// Sequence number of the latest ledger at time of request.
	LatestLedger int64 `json:"latestLedger,string"`
}

const getLedgerEntriesMaxKeys = 200

// NewGetLedgerEntriesHandler returns a JSON RPC handler to retrieve the specified ledger entries from Stellar Core.
func NewGetLedgerEntriesHandler(logger *log.Entry, ledgerEntryReader db.LedgerEntryReader) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request GetLedgerEntriesRequest) (GetLedgerEntriesResponse, error) {
		if len(request.Keys) > getLedgerEntriesMaxKeys {
			return GetLedgerEntriesResponse{}, &jrpc2.Error{
				Code:    jrpc2.InvalidParams,
				Message: fmt.Sprintf("key count (%d) exceeds maximum supported (%d)", len(request.Keys), getLedgerEntriesMaxKeys),
			}
		}
		var ledgerKeys []xdr.LedgerKey
		for i, requestKey := range request.Keys {
			var ledgerKey xdr.LedgerKey
			if err := gxdr.ValidateLedgerKey(requestKey, gxdr.DefaultMaxDepth); err != nil {
				logger.WithError(err).WithField("request", request).
					Infof("could not validate ledgerKey at index %d from getLedgerEntries request", i)
				return GetLedgerEntriesResponse{}, &jrpc2.Error{
					Code:    jrpc2.InvalidParams,
					Message: fmt.Sprintf("cannot unmarshal key value %s at index %d", requestKey, i),
				}
			}
			if err := xdr.SafeUnmarshalBase64(requestKey, &ledgerKey); err != nil {
				logger.WithError(err).WithField("request", request).
					Infof("could not unmarshal requestKey %s at index %d from getLedgerEntries request", requestKey, i)
				return GetLedgerEntriesResponse{}, &jrpc2.Error{
					Code:    jrpc2.InvalidParams,
					Message: fmt.Sprintf("cannot unmarshal key value %s at index %d", requestKey, i),
				}
			}
			ledgerKeys = append(ledgerKeys, ledgerKey)
		}

		tx, err := ledgerEntryReader.NewTx(ctx)
		if err != nil {
			return GetLedgerEntriesResponse{}, &jrpc2.Error{
				Code:    jrpc2.InternalError,
				Message: "could not create read transaction",
			}
		}
		defer func() {
			_ = tx.Done()
		}()

		latestLedger, err := tx.GetLatestLedgerSequence()
		if err != nil {
			return GetLedgerEntriesResponse{}, &jrpc2.Error{
				Code:    jrpc2.InternalError,
				Message: "could not get latest ledger",
			}
		}

		ledgerEntryResults := make([]LedgerEntryResult, 0, len(ledgerKeys))
		ledgerKeysAndEntries, err := tx.GetLedgerEntries(ledgerKeys...)
		if err != nil {
			message := "could not obtain ledger entries from storage"
			logger.WithError(err).WithField("request", request).Info(message)
			return GetLedgerEntriesResponse{}, &jrpc2.Error{
				Code:    jrpc2.InternalError,
				Message: message,
			}
		}

		for _, ledgerKeyAndEntry := range ledgerKeysAndEntries {
			entryXdr, err := xdr.MarshalBase64(ledgerKeyAndEntry.Entry.Data)
			if err != nil {
				message := fmt.Sprintf(
					"could not serialize ledger entry data for ledger entry %v",
					ledgerKeyAndEntry.Entry)
				logger.WithError(err).WithField("request", request).Info(message)
				return GetLedgerEntriesResponse{}, &jrpc2.Error{
					Code:    jrpc2.InternalError,
					Message: message,
				}
			}

			keyXdr, err := xdr.MarshalBase64(ledgerKeyAndEntry.Key)
			if err != nil {
				message := fmt.Sprintf("could not serialize ledger key %v", ledgerKeyAndEntry.Key)
				logger.WithError(err).WithField("request", request).Infof(message)
				return GetLedgerEntriesResponse{}, &jrpc2.Error{
					Code:    jrpc2.InternalError,
					Message: message,
				}
			}

			ledgerEntryResults = append(ledgerEntryResults, LedgerEntryResult{
				Key:                keyXdr,
				XDR:                entryXdr,
				LastModifiedLedger: int64(ledgerKeyAndEntry.Entry.LastModifiedLedgerSeq),
			})
		}

		response := GetLedgerEntriesResponse{
			Entries:      ledgerEntryResults,
			LatestLedger: int64(latestLedger),
		}
		return response, nil
	})
}
