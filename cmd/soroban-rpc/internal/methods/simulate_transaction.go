package methods

import (
	"context"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/preflight"
)

type SimulateTransactionRequest struct {
	Transaction string `json:"transaction"`
}

type SimulateTransactionCost struct {
	CPUInstructions uint64 `json:"cpuInsns,string"`
	MemoryBytes     uint64 `json:"memBytes,string"`
}

type SimulateTransactionResult struct {
	Auth      []string `json:"auth"`
	Footprint string   `json:"footprint"`
	XDR       string   `json:"xdr"`
}

type SimulateTransactionResponse struct {
	Error        string                      `json:"error,omitempty"`
	Results      []SimulateTransactionResult `json:"results,omitempty"`
	Cost         SimulateTransactionCost     `json:"cost"`
	LatestLedger int64                       `json:"latestLedger,string"`
}

type PreflightWorkerPool interface {
	GetPreflight(ctx context.Context, readTx db.LedgerEntryReadTx, sourceAccount xdr.AccountId, op xdr.InvokeHostFunctionOp) (preflight.Preflight, error)
	GetJobQueueLenAndCapacity() (uint, uint)
}

// NewSimulateTransactionHandler returns a json rpc handler to run preflight simulations
func NewSimulateTransactionHandler(logger *log.Entry, ledgerEntryReader db.LedgerEntryReader, pwp PreflightWorkerPool) jrpc2.Handler {
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
		op := txEnvelope.Operations()[0]

		var sourceAccount xdr.AccountId
		if opSourceAccount := op.SourceAccount; opSourceAccount != nil {
			sourceAccount = opSourceAccount.ToAccountId()
		} else {
			// FIXME: SourceAccount() panics, so, the user can doctor an envelope which makes the server crash
			sourceAccount = txEnvelope.SourceAccount().ToAccountId()
		}

		xdrOp, ok := op.Body.GetInvokeHostFunctionOp()
		if !ok {
			return SimulateTransactionResponse{
				Error: "Transaction does not contain invoke host function operation",
			}
		}

		jobBufferLen, jobBufferCapacity := pwp.GetJobQueueLenAndCapacity()
		if jobBufferLen != 0 && jobBufferLen == jobBufferCapacity {
			return SimulateTransactionResponse{
				Error: "All workers are busy, try again later",
			}
		}

		readTx, err := ledgerEntryReader.NewTx(ctx)
		if err != nil {
			return SimulateTransactionResponse{
				Error: "Cannot create read transaction",
			}
		}
		defer func() {
			_ = readTx.Done()
		}()
		latestLedger, err := readTx.GetLatestLedgerSequence()
		if err != nil {
			return SimulateTransactionResponse{
				Error: err.Error(),
			}
		}

		result, err := pwp.GetPreflight(ctx, readTx, sourceAccount, xdrOp)
		if err != nil {
			return SimulateTransactionResponse{
				Error:        err.Error(),
				LatestLedger: int64(latestLedger),
			}
		}

		return SimulateTransactionResponse{
			Results: []SimulateTransactionResult{
				{
					Auth:      result.Auth,
					Footprint: result.Footprint,
					XDR:       result.Result,
				},
			},
			Cost: SimulateTransactionCost{
				CPUInstructions: result.CPUInstructions,
				MemoryBytes:     result.MemoryBytes,
			},
			LatestLedger: int64(latestLedger),
		}
	})
}
