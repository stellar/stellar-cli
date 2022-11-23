package methods

import (
	"context"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
)

type HealthCheckResult struct {
	Status string `json:"status"`
}

// NewHealthCheck returns a health check json rpc handler
func NewHealthCheck() jrpc2.Handler {
	return handler.New(func(context.Context) HealthCheckResult {
		return HealthCheckResult{Status: "healthy"}
	})
}
