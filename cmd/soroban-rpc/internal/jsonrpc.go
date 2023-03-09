package internal

import (
	"net/http"

	"github.com/creachadair/jrpc2/handler"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/rs/cors"

	"github.com/stellar/go/clients/stellarcore"
	"github.com/stellar/go/support/log"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/events"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

// Handler is the HTTP handler which serves the Soroban JSON RPC responses
type Handler struct {
	bridge jhttp.Bridge
	logger *log.Entry
	http.Handler
}

// Close closes all the resources held by the Handler instances.
// After Close is called the Handler instance will stop accepting JSON RPC requests.
func (h Handler) Close() {
	if err := h.bridge.Close(); err != nil {
		h.logger.WithError(err).Warn("could not close bridge")
	}
}

type HandlerParams struct {
	EventStore        *events.MemoryStore
	TransactionStore  *transactions.MemoryStore
	CoreClient        *stellarcore.Client
	LedgerEntryReader db.LedgerEntryReader
	Logger            *log.Entry
	PreflightGetter   methods.PreflightGetter
}

// NewJSONRPCHandler constructs a Handler instance
func NewJSONRPCHandler(cfg *config.LocalConfig, params HandlerParams) (Handler, error) {
	bridge := jhttp.NewBridge(handler.Map{
		"getHealth":           methods.NewHealthCheck(params.TransactionStore, cfg.MaxHealthyLedgerLatency),
		"getEvents":           methods.NewGetEventsHandler(params.EventStore, cfg.MaxEventsLimit, cfg.DefaultEventsLimit),
		"getNetwork":          methods.NewGetNetworkHandler(cfg.NetworkPassphrase, cfg.FriendbotURL, params.CoreClient),
		"getLedgerEntry":      methods.NewGetLedgerEntryHandler(params.Logger, params.LedgerEntryReader),
		"getTransaction":      methods.NewGetTransactionHandler(params.TransactionStore),
		"sendTransaction":     methods.NewSendTransactionHandler(params.Logger, params.TransactionStore, cfg.NetworkPassphrase, params.CoreClient),
		"simulateTransaction": methods.NewSimulateTransactionHandler(params.Logger, params.PreflightGetter),
	}, nil)
	corsMiddleware := cors.New(cors.Options{
		AllowedOrigins: []string{"*"},
		AllowedHeaders: []string{"*"},
		AllowedMethods: []string{"GET", "PUT", "POST", "PATCH", "DELETE", "HEAD", "OPTIONS"},
	})

	return Handler{
		bridge:  bridge,
		logger:  params.Logger,
		Handler: corsMiddleware.Handler(bridge),
	}, nil
}
