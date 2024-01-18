package methods

import (
	"context"
	"fmt"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

type HealthCheckResult struct {
	Status string `json:"status"`
}

// NewHealthCheck returns a health check json rpc handler
func NewHealthCheck(txStore *transactions.MemoryStore, maxHealthyLedgerLatency time.Duration) jrpc2.Handler {
	return handler.New(func(ctx context.Context) (HealthCheckResult, error) {
		ledgerInfo := txStore.GetLatestLedger()
		if ledgerInfo.Sequence < 1 {
			return HealthCheckResult{}, jrpc2.Error{
				Code:    jrpc2.InternalError,
				Message: "data stores are not initialized",
			}
		}
		lastKnownLedgerCloseTime := time.Unix(ledgerInfo.CloseTime, 0)
		lastKnownLedgerLatency := time.Since(lastKnownLedgerCloseTime)
		if lastKnownLedgerLatency > maxHealthyLedgerLatency {
			roundedLatency := lastKnownLedgerLatency.Round(time.Second)
			msg := fmt.Sprintf("latency (%s) since last known ledger closed is too high (>%s)", roundedLatency, maxHealthyLedgerLatency)
			return HealthCheckResult{}, jrpc2.Error{
				Code:    jrpc2.InternalError,
				Message: msg,
			}
		}
		return HealthCheckResult{Status: "healthy"}, nil
	})
}
