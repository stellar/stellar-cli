package internal

import (
	"context"
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
)

// Handler is the HTTP handler which serves the Soroban JSON RPC responses
type Handler struct {
	bridge           jhttp.Bridge
	logger           *log.Entry
	transactionProxy *methods.TransactionProxy
	http.Handler
}

// Start spawns the background workers necessary for the JSON RPC handlers.
func (h Handler) Start() {
	h.transactionProxy.Start(context.Background())
}

// Close closes all the resources held by the Handler instances.
// After Close is called the Handler instance will stop accepting JSON RPC requests.
func (h Handler) Close() {
	if err := h.bridge.Close(); err != nil {
		h.logger.WithError(err).Warn("could not close bridge")
	}
	h.transactionProxy.Close()
}

type HandlerParams struct {
	AccountStore      methods.AccountStore
	EventStore        *events.MemoryStore
	TransactionProxy  *methods.TransactionProxy
	CoreClient        *stellarcore.Client
	LedgerEntryReader db.LedgerEntryReader
	Logger            *log.Entry
}

// NewJSONRPCHandler constructs a Handler instance
func NewJSONRPCHandler(cfg config.LocalConfig, params HandlerParams) (Handler, error) {
	bridge := jhttp.NewBridge(handler.Map{
		"getHealth":            methods.NewHealthCheck(),
		"getAccount":           methods.NewAccountHandler(params.AccountStore),
		"getEvents":            methods.NewGetEventsHandler(params.EventStore, cfg.MaxEventsLimit, cfg.DefaultEventsLimit),
		"getNetwork":           methods.NewGetNetworkHandler(cfg.NetworkPassphrase, cfg.FriendbotURL, params.CoreClient),
		"getLedgerEntry":       methods.NewGetLedgerEntryHandler(params.Logger, params.LedgerEntryReader),
		"getTransactionStatus": methods.NewGetTransactionStatusHandler(params.TransactionProxy),
		"sendTransaction":      methods.NewSendTransactionHandler(params.TransactionProxy),
		"simulateTransaction":  methods.NewSimulateTransactionHandler(params.Logger, cfg.NetworkPassphrase, params.LedgerEntryReader),
	}, nil)
	corsMiddleware := cors.New(cors.Options{
		AllowedOrigins: []string{"*"},
		AllowedHeaders: []string{"*"},
		AllowedMethods: []string{"GET", "PUT", "POST", "PATCH", "DELETE", "HEAD", "OPTIONS"},
	})

	return Handler{
		bridge:           bridge,
		logger:           params.Logger,
		transactionProxy: params.TransactionProxy,
		Handler:          corsMiddleware.Handler(bridge),
	}, nil
}
