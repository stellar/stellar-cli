package methods

import (
	"context"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"

	"github.com/stellar/go/clients/stellarcore"
	proto "github.com/stellar/go/protocols/stellarcore"
	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"
)

type GetLedgerEntryRequest struct {
	Key string `json:"key"`
}

type GetLedgerEntryResponse struct {
	XDR                string `json:"xdr"`
	LastModifiedLedger int64  `json:"lastModifiedLedgerSeq,string"`
	LatestLedger       int64  `json:"latestLedger,string"`
}

// NewGetLedgerEntryHandler returns a json rpc handler to retrieve the specified ledger entry from stellar core
func NewGetLedgerEntryHandler(logger *log.Entry, coreClient *stellarcore.Client) jrpc2.Handler {
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

		coreResponse, err := coreClient.GetLedgerEntry(ctx, key)
		if err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not submit getLedgerEntry request to core")
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not submit request to core",
			}
		}

		if coreResponse.State == proto.DeadState {
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InvalidRequest,
				Message: "not found",
			}
		}

		var ledgerEntry xdr.LedgerEntry
		if err = xdr.SafeUnmarshalBase64(coreResponse.Entry, &ledgerEntry); err != nil {
			logger.WithError(err).WithField("request", request).
				WithField("response", coreResponse).
				Info("could not parse ledger entry")
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not parse core response",
			}
		}

		response := GetLedgerEntryResponse{
			LastModifiedLedger: int64(ledgerEntry.LastModifiedLedgerSeq),
			LatestLedger:       coreResponse.Ledger,
		}
		if response.XDR, err = xdr.MarshalBase64(ledgerEntry.Data); err != nil {
			logger.WithError(err).WithField("request", request).
				WithField("response", coreResponse).
				Info("could not serialize ledger entry data")
			return GetLedgerEntryResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not serialize ledger entry data",
			}
		}

		return response, nil
	})
}
