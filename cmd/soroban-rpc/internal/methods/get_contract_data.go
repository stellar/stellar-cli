package methods

import (
	"context"
	"encoding/hex"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"

	"github.com/stellar/go/clients/stellarcore"
	proto "github.com/stellar/go/protocols/stellarcore"
	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"
)

type GetContractDataRequest struct {
	ContractID string `json:"contractId"`
	Key        string `json:"key"`
}

type GetContractDataResponse struct {
	XDR                string `json:"xdr"`
	LastModifiedLedger int64  `json:"lastModifiedLedgerSeq,string"`
	LatestLedger       int64  `json:"latestLedger,string"`
}

// NewGetContractDataHandler returns a json rpc handler to retrieve a contract data ledger entry from stellar cre
func NewGetContractDataHandler(logger *log.Entry, coreClient *stellarcore.Client) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request GetContractDataRequest) (GetContractDataResponse, error) {
		var scVal xdr.ScVal
		if err := xdr.SafeUnmarshalBase64(request.Key, &scVal); err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not unmarshal scVal from getContractData request")
			return GetContractDataResponse{}, &jrpc2.Error{
				Code:    code.InvalidParams,
				Message: "cannot unmarshal key value",
			}
		}
		contractIDBytes, err := hex.DecodeString(request.ContractID)
		if err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not unmarshal contract id from getContractData request")
			return GetContractDataResponse{}, &jrpc2.Error{
				Code:    code.InvalidParams,
				Message: "cannot unmarshal contract id",
			}
		}
		if len(contractIDBytes) != 32 {
			return GetContractDataResponse{}, &jrpc2.Error{
				Code:    code.InvalidParams,
				Message: "contract id is not 32 bytes",
			}
		}
		var contractId xdr.Hash
		copy(contractId[:], contractIDBytes)
		lk := xdr.LedgerKey{
			Type: xdr.LedgerEntryTypeContractData,
			ContractData: &xdr.LedgerKeyContractData{
				ContractId: contractId,
				Key:        scVal,
			},
		}

		coreResponse, err := coreClient.GetLedgerEntry(ctx, lk)
		if err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not submit getLedgerEntry request to core")
			return GetContractDataResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not submit request to core",
			}
		}

		if coreResponse.State == proto.DeadState {
			return GetContractDataResponse{}, &jrpc2.Error{
				Code:    code.InvalidRequest,
				Message: "not found",
			}
		}

		var ledgerEntry xdr.LedgerEntry
		if err = xdr.SafeUnmarshalBase64(coreResponse.Entry, &ledgerEntry); err != nil {
			logger.WithError(err).WithField("request", request).
				WithField("response", coreResponse).
				Info("could not parse ledger entry")
			return GetContractDataResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not parse core response",
			}
		}

		contractData, ok := ledgerEntry.Data.GetContractData()
		if !ok {
			logger.WithError(err).WithField("request", request).
				WithField("response", coreResponse).
				Info("ledger entry does not contain contract data")
			return GetContractDataResponse{}, &jrpc2.Error{
				Code:    code.InvalidRequest,
				Message: "ledger entry does not contain contract data",
			}
		}
		response := GetContractDataResponse{
			LastModifiedLedger: int64(ledgerEntry.LastModifiedLedgerSeq),
			LatestLedger:       coreResponse.Ledger,
		}
		if response.XDR, err = xdr.MarshalBase64(contractData.Val); err != nil {
			logger.WithError(err).WithField("request", request).
				WithField("response", coreResponse).
				Info("could not serialize contract data scval")
			return GetContractDataResponse{}, &jrpc2.Error{
				Code:    code.InternalError,
				Message: "could not serialize contract data scval",
			}
		}

		return response, nil
	})
}
