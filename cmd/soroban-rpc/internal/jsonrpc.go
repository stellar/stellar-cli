package internal

import (
	"context"
	"encoding/json"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/go-chi/chi/middleware"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/support/log"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/events"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/network"
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
	LedgerEntryReader db.LedgerEntryReader
	LedgerReader      db.LedgerReader
	Logger            *log.Entry
	PreflightGetter   methods.PreflightGetter
	Daemon            interfaces.Daemon
}

func decorateHandlers(daemon interfaces.Daemon, logger *log.Entry, m handler.Map) handler.Map {
	requestMetric := prometheus.NewSummaryVec(prometheus.SummaryOpts{
		Namespace: daemon.MetricsNamespace(),
		Subsystem: "json_rpc",
		Name:      "request_duration_seconds",
		Help:      "JSON RPC request duration",
	}, []string{"endpoint", "status"})
	decorated := handler.Map{}
	for endpoint, h := range m {
		// create copy of h so it can be used in closure bleow
		h := h
		decorated[endpoint] = handler.New(func(ctx context.Context, r *jrpc2.Request) (interface{}, error) {
			reqID := strconv.FormatUint(middleware.NextRequestID(), 10)
			logRequest(logger, reqID, r)
			startTime := time.Now()
			result, err := h.Handle(ctx, r)
			duration := time.Since(startTime)
			label := prometheus.Labels{"endpoint": r.Method(), "status": "ok"}
			simulateTransactionResponse, ok := result.(methods.SimulateTransactionResponse)
			if ok && simulateTransactionResponse.Error != "" {
				label["status"] = "error"
			} else if err != nil {
				if jsonRPCErr, ok := err.(*jrpc2.Error); ok {
					prometheusLabelReplacer := strings.NewReplacer(" ", "_", "-", "_", "(", "", ")", "")
					status := prometheusLabelReplacer.Replace(jsonRPCErr.Code.String())
					label["status"] = status
				}
			}
			requestMetric.With(label).Observe(duration.Seconds())
			logResponse(logger, reqID, duration, label["status"], result)
			return result, err
		})
	}
	daemon.MetricsRegistry().MustRegister(requestMetric)
	return decorated
}

func logRequest(logger *log.Entry, reqID string, req *jrpc2.Request) {
	logger = logger.WithFields(log.F{
		"subsys":   "jsonrpc",
		"req":      reqID,
		"json_req": req.ID(),
		"method":   req.Method(),
	})
	logger.Info("starting JSONRPC request")

	// Params are useful but can be really verbose, let's only print them in debug level
	logger = logger.WithField("params", req.ParamString())
	logger.Debug("starting JSONRPC request params")
}

func logResponse(logger *log.Entry, reqID string, duration time.Duration, status string, response any) {
	logger = logger.WithFields(log.F{
		"subsys":   "jsonrpc",
		"req":      reqID,
		"duration": duration.String(),
		"json_req": reqID,
		"status":   status,
	})
	logger.Info("finished JSONRPC request")

	if status == "ok" {
		responseBytes, err := json.Marshal(response)
		if err == nil {
			// the result is useful but can be really verbose, let's only print it with debug level
			logger = logger.WithField("result", string(responseBytes))
			logger.Debug("finished JSONRPC request result")
		}
	}
}

// NewJSONRPCHandler constructs a Handler instance
func NewJSONRPCHandler(cfg *config.Config, params HandlerParams) Handler {
	bridgeOptions := jhttp.BridgeOptions{
		Server: &jrpc2.ServerOptions{
			Logger: func(text string) { params.Logger.Debug(text) },
		},
	}
	handlers := []struct {
		methodName        string
		underlyingHandler jrpc2.Handler
		gaugeName         string
		gaugeHelp         string
		queueLimit        uint
	}{
		{
			methodName:        "getHealth",
			underlyingHandler: methods.NewHealthCheck(params.TransactionStore, cfg.MaxHealthyLedgerLatency),
			gaugeName:         "get_health_inflight_requests",
			gaugeHelp:         "Number of concurrenty in-flight getHealth requests",
			queueLimit:        cfg.RequestBacklogGetHealthQueueLimit,
		},
		{
			methodName:        "getEvents",
			underlyingHandler: methods.NewGetEventsHandler(params.EventStore, cfg.MaxEventsLimit, cfg.DefaultEventsLimit),
			gaugeName:         "get_events_inflight_requests",
			gaugeHelp:         "Number of concurrenty in-flight getEvents requests",
			queueLimit:        cfg.RequestBacklogGetEventsQueueLimit,
		},
		{
			methodName:        "getNetwork",
			underlyingHandler: methods.NewGetNetworkHandler(params.Daemon, cfg.NetworkPassphrase, cfg.FriendbotURL),
			gaugeName:         "get_network_inflight_requests",
			gaugeHelp:         "Number of concurrenty in-flight getNetwork requests",
			queueLimit:        cfg.RequestBacklogGetNetworkQueueLimit,
		},
		{
			methodName:        "getLatestLedger",
			underlyingHandler: methods.NewGetLatestLedgerHandler(params.LedgerEntryReader, params.LedgerReader),
			gaugeName:         "get_latest_ledger_inflight_requests",
			gaugeHelp:         "Number of concurrenty in-flight getLatestLedger requests",
			queueLimit:        cfg.RequestBacklogGetLatestLedgerQueueLimit,
		},
		{
			methodName:        "getLedgerEntry",
			underlyingHandler: methods.NewGetLedgerEntryHandler(params.Logger, params.LedgerEntryReader),
			gaugeName:         "get_ledger_entry_inflight_requests",
			gaugeHelp:         "Number of concurrenty in-flight getLedgerEntry requests",
			queueLimit:        cfg.RequestBacklogGetLedgerEntriesQueueLimit, // share with getLedgerEntries
		},
		{
			methodName:        "getLedgerEntries",
			underlyingHandler: methods.NewGetLedgerEntriesHandler(params.Logger, params.LedgerEntryReader),
			gaugeName:         "get_ledger_entries_inflight_requests",
			gaugeHelp:         "Number of concurrenty in-flight getLedgerEntries requests",
			queueLimit:        cfg.RequestBacklogGetLedgerEntriesQueueLimit,
		},
		{
			methodName:        "getTransaction",
			underlyingHandler: methods.NewGetTransactionHandler(params.TransactionStore),
			gaugeName:         "get_transaction_inflight_requests",
			gaugeHelp:         "Number of concurrenty in-flight getTransactions requests",
			queueLimit:        cfg.RequestBacklogGetTransactionQueueLimit,
		},
		{
			methodName:        "sendTransaction",
			underlyingHandler: methods.NewSendTransactionHandler(params.Daemon, params.Logger, params.TransactionStore, cfg.NetworkPassphrase),
			gaugeName:         "send_transaction_inflight_requests",
			gaugeHelp:         "Number of concurrenty in-flight sendTransactions requests",
			queueLimit:        cfg.RequestBacklogSendTransactionQueueLimit,
		},
		{
			methodName:        "simulateTransaction",
			underlyingHandler: methods.NewSimulateTransactionHandler(params.Logger, params.LedgerEntryReader, params.PreflightGetter),
			gaugeName:         "simulate_transaction_inflight_requests",
			gaugeHelp:         "Number of concurrenty in-flight simulateTransactions requests",
			queueLimit:        cfg.RequestBacklogSimulateTransactionQueueLimit,
		},
	}
	handlersMap := handler.Map{}
	for _, handler := range handlers {
		gauge := prometheus.NewGauge(prometheus.GaugeOpts{
			Namespace: params.Daemon.MetricsNamespace(), Subsystem: "network",
			Name: handler.gaugeName,
			Help: handler.gaugeHelp,
		})
		limiter := network.MakeJrpcBacklogQueueLimiter(
			handler.underlyingHandler,
			gauge,
			uint64(handler.queueLimit),
			params.Logger)
		handlersMap[handler.methodName] = limiter
	}
	bridge := jhttp.NewBridge(decorateHandlers(
		params.Daemon,
		params.Logger,
		handlersMap),
		&bridgeOptions)

	// globalQueueRequestBacklogLimiter is a metric for measuring the total concurrent inflight requests
	globalQueueRequestBacklogLimiter := prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: params.Daemon.MetricsNamespace(), Subsystem: "network", Name: "global_inflight_requests",
		Help: "Number of concurrenty in-flight http requests",
	})
	return Handler{
		bridge: bridge,
		logger: params.Logger,
		Handler: network.MakeHTTPBacklogQueueLimiter(
			bridge,
			globalQueueRequestBacklogLimiter,
			uint64(cfg.RequestBacklogGlobalQueueLimit),
			params.Logger),
	}
}
