package methods

import (
	"context"

	"github.com/stellar/go/clients/stellarcore"
	proto "github.com/stellar/go/protocols/stellarcore"
	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
)

type SimulateTransactionRequest struct {
	Transaction string `json:"transaction"`
}

type SimulateTransactionCost struct {
	CPUInstructions uint64 `json:"cpuInsns,string"`
	MemoryBytes     uint64 `json:"memBytes,string"`
}

type InvokeHostFunctionResult struct {
	XDR string `json:"xdr"`
}

type SimulateTransactionResponse struct {
	Error        string                     `json:"error,omitempty"`
	Results      []InvokeHostFunctionResult `json:"results,omitempty"`
	Footprint    string                     `json:"footprint"`
	Cost         SimulateTransactionCost    `json:"cost"`
	LatestLedger int64                      `json:"latestLedger,string"`
}

// NewSimulateTransactionHandler returns a json rpc handler to execute preflight requests to stellar core
func NewSimulateTransactionHandler(logger *log.Entry, coreClient *stellarcore.Client) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request SimulateTransactionRequest) SimulateTransactionResponse {
		var txEnvelope xdr.TransactionEnvelope
		if err := xdr.SafeUnmarshalBase64(request.Transaction, &txEnvelope); err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not unmarshal simulate transaction envelope")
			return SimulateTransactionResponse{
				Error: "Could not unmarshal transaction",
			}
		}
		if len(txEnvelope.Operations()) != 1 {
			return SimulateTransactionResponse{
				Error: "Transaction contains more than one operation",
			}
		}

		var sourceAccount string
		if opSourceAccount := txEnvelope.Operations()[0].SourceAccount; opSourceAccount != nil {
			sourceAccount = opSourceAccount.ToAccountId().Address()
		} else {
			sourceAccount = txEnvelope.SourceAccount().ToAccountId().Address()
		}

		xdrOp, ok := txEnvelope.Operations()[0].Body.GetInvokeHostFunctionOp()
		if !ok {
			return SimulateTransactionResponse{
				Error: "Transaction does not contain invoke host function operation",
			}
		}

		coreResponse, err := coreClient.Preflight(ctx, sourceAccount, xdrOp)
		if err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not submit preflight request to core")
			return SimulateTransactionResponse{
				Error: "Could not submit request to core",
			}
		}

		if coreResponse.Status == proto.PreflightStatusError {
			return SimulateTransactionResponse{
				Error:        coreResponse.Detail,
				LatestLedger: coreResponse.Ledger,
			}
		}

		return SimulateTransactionResponse{
			Results:   []InvokeHostFunctionResult{{XDR: coreResponse.Result}},
			Footprint: coreResponse.Footprint,
			Cost: SimulateTransactionCost{
				CPUInstructions: coreResponse.CPUInstructions,
				MemoryBytes:     coreResponse.MemoryBytes,
			},
			LatestLedger: coreResponse.Ledger,
		}
	})
}
