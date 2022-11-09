package internal

import (
	"context"
	"net/http"

	"github.com/creachadair/jrpc2/handler"
	"github.com/creachadair/jrpc2/jhttp"

	"github.com/stellar/go/ingest/ledgerbackend"
	"github.com/stellar/go/support/log"
)

// Handler is the HTTP handler which serves the Soroban JSON RPC responses
type Handler struct {
	bridge jhttp.Bridge
	core   *ledgerbackend.CaptiveStellarCore
	logger *log.Entry
}

// ServeHTTP implements the http.Handler interface
func (h Handler) ServeHTTP(w http.ResponseWriter, req *http.Request) {
	h.bridge.ServeHTTP(w, req)
}

// Close closes all of the resources held by the Handler instances.
// After Close is called the Handler instance will stop accepting JSON RPC requests.
func (h Handler) Close() {
	if err := h.core.Close(); err != nil {
		h.logger.WithError(err).Warn("could not close captive core")
	}
	if err := h.bridge.Close(); err != nil {
		h.logger.WithError(err).Warn("could not close bridge")
	}
}

type HealthCheckResult struct {
	Status string `json:"status"`
}

// NewJSONRPCHandler constructs a Handler instance
func NewJSONRPCHandler(captiveConfig ledgerbackend.CaptiveCoreConfig, logger *log.Entry) (Handler, error) {
	core, err := ledgerbackend.NewCaptive(captiveConfig)
	if err != nil {
		return Handler{}, err
	}

	return Handler{
		bridge: jhttp.NewBridge(handler.Map{
			"getHealth": handler.New(func(ctx context.Context) HealthCheckResult {
				return HealthCheckResult{Status: "healthy"}
			}),
		}, nil),
		core:   core,
		logger: logger,
	}, nil
}
