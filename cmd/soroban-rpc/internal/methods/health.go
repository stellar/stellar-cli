package methods

import (
	"context"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/handler"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

type HealthCheckResult struct {
	Status string `json:"status"`
}

// NewHealthCheck returns a health check json rpc handler
func NewHealthCheck(reader db.LedgerEntryReader) jrpc2.Handler {
	return handler.New(func(ctx context.Context) (HealthCheckResult, error) {
		if _, err := reader.GetLatestLedgerSequence(ctx); err != nil {
			return HealthCheckResult{}, jrpc2.Error{
				Code:    code.InternalError,
				Message: err.Error(),
			}
		}
		return HealthCheckResult{Status: "healthy"}, nil
	})
}
