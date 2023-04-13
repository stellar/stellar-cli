package internal

import (
	"context"
	"net/http"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/go-chi/chi/middleware"
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
	LedgerReader      db.LedgerReader
	Logger            *log.Entry
	PreflightGetter   methods.PreflightGetter
}

// NewJSONRPCHandler constructs a Handler instance
func NewJSONRPCHandler(cfg *config.LocalConfig, params HandlerParams) Handler {
	bridgeOptions := jhttp.BridgeOptions{
		Server: &jrpc2.ServerOptions{
			Logger: func(text string) { params.Logger.Debug(text) },
			RPCLog: &rpcLogger{logger: params.Logger},
		},
	}
	bridge := jhttp.NewBridge(handler.Map{
		"getHealth":           methods.NewHealthCheck(params.TransactionStore, cfg.MaxHealthyLedgerLatency),
		"getEvents":           methods.NewGetEventsHandler(params.EventStore, cfg.MaxEventsLimit, cfg.DefaultEventsLimit),
		"getNetwork":          methods.NewGetNetworkHandler(cfg.NetworkPassphrase, cfg.FriendbotURL, params.CoreClient),
		"getLatestLedger":     methods.NewGetLatestLedgerHandler(params.LedgerEntryReader, params.LedgerReader),
		"getLedgerEntry":      methods.NewGetLedgerEntryHandler(params.Logger, params.LedgerEntryReader),
		"getTransaction":      methods.NewGetTransactionHandler(params.TransactionStore),
		"sendTransaction":     methods.NewSendTransactionHandler(params.Logger, params.TransactionStore, cfg.NetworkPassphrase, params.CoreClient),
		"simulateTransaction": methods.NewSimulateTransactionHandler(params.Logger, params.LedgerEntryReader, params.PreflightGetter),
	}, &bridgeOptions)

	return Handler{
		bridge:  bridge,
		logger:  params.Logger,
		Handler: bridge,
	}
}

type rpcLogger struct {
	logger *log.Entry
}

func (r *rpcLogger) LogRequest(ctx context.Context, req *jrpc2.Request) {
	r.logger.WithFields(log.F{
		"subsys":   "jsonrpc",
		"req":      middleware.GetReqID(ctx),
		"json_req": req.ID(),
		"method":   req.Method(),
		// TODO: is this overkill?
		"params": req.ParamString(),
	}).Info("starting JSONRPC request")
}

func (r *rpcLogger) LogResponse(ctx context.Context, rsp *jrpc2.Response) {
	// TODO: Print the elapsed time (there doesn't seem to be a way to it with with jrpc2, since
	//       LogRequest cannot modify the context)
	logger := r.logger.WithFields(log.F{
		"subsys":   "jsonrpc",
		"req":      middleware.GetReqID(ctx),
		"json_req": rsp.ID(),
		// TODO: is this overkill?
		"result": rsp.ResultString(),
	})
	if err := rsp.Error(); err != nil {
		logger = logger.WithField("error", err.Error())
	}
	logger.Info("finished JSONRPC request")
}
