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
	"github.com/rs/cors"
	"github.com/stellar/go/support/log"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/events"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/network"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

// maxHTTPRequestSize defines the largest request size that the http handler
// would be willing to accept before dropping the request. The implementation
// uses the default MaxBytesHandler to limit the request size.
const maxHTTPRequestSize = 512 * 1024 // half a megabyte

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
		Namespace:  daemon.MetricsNamespace(),
		Subsystem:  "json_rpc",
		Name:       "request_duration_seconds",
		Help:       "JSON RPC request duration",
		Objectives: map[float64]float64{0.5: 0.05, 0.9: 0.01, 0.99: 0.001},
	}, []string{"endpoint", "status"})
	decorated := handler.Map{}
	for endpoint, h := range m {
		// create copy of h so it can be used in closure bleow
		h := h
		decorated[endpoint] = handler.New(func(ctx context.Context, r *jrpc2.Request) (interface{}, error) {
			reqID := strconv.FormatUint(middleware.NextRequestID(), 10)
			logRequest(logger, reqID, r)
			startTime := time.Now()
			result, err := h(ctx, r)
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
		methodName           string
		underlyingHandler    jrpc2.Handler
		queueLimit           uint
		longName             string
		requestDurationLimit time.Duration
	}{
		{
			methodName:           "getHealth",
			underlyingHandler:    methods.NewHealthCheck(params.TransactionStore, cfg.MaxHealthyLedgerLatency),
			longName:             "get_health",
			queueLimit:           cfg.RequestBacklogGetHealthQueueLimit,
			requestDurationLimit: cfg.MaxGetHealthExecutionDuration,
		},
		{
			methodName:           "getEvents",
			underlyingHandler:    methods.NewGetEventsHandler(params.EventStore, cfg.MaxEventsLimit, cfg.DefaultEventsLimit),
			longName:             "get_events",
			queueLimit:           cfg.RequestBacklogGetEventsQueueLimit,
			requestDurationLimit: cfg.MaxGetEventsExecutionDuration,
		},
		{
			methodName:           "getNetwork",
			underlyingHandler:    methods.NewGetNetworkHandler(params.Daemon, cfg.NetworkPassphrase, cfg.FriendbotURL),
			longName:             "get_network",
			queueLimit:           cfg.RequestBacklogGetNetworkQueueLimit,
			requestDurationLimit: cfg.MaxGetNetworkExecutionDuration,
		},
		{
			methodName:           "getLatestLedger",
			underlyingHandler:    methods.NewGetLatestLedgerHandler(params.LedgerEntryReader, params.LedgerReader),
			longName:             "get_latest_ledger",
			queueLimit:           cfg.RequestBacklogGetLatestLedgerQueueLimit,
			requestDurationLimit: cfg.MaxGetLatestLedgerExecutionDuration,
		},
		{
			methodName:           "getLedgerEntry",
			underlyingHandler:    methods.NewGetLedgerEntryHandler(params.Logger, params.LedgerEntryReader),
			longName:             "get_ledger_entry",
			queueLimit:           cfg.RequestBacklogGetLedgerEntriesQueueLimit, // share with getLedgerEntries
			requestDurationLimit: cfg.MaxGetLedgerEntriesExecutionDuration,
		},
		{
			methodName:           "getLedgerEntries",
			underlyingHandler:    methods.NewGetLedgerEntriesHandler(params.Logger, params.LedgerEntryReader),
			longName:             "get_ledger_entries",
			queueLimit:           cfg.RequestBacklogGetLedgerEntriesQueueLimit,
			requestDurationLimit: cfg.MaxGetLedgerEntriesExecutionDuration,
		},
		{
			methodName:           "getTransaction",
			underlyingHandler:    methods.NewGetTransactionHandler(params.TransactionStore),
			longName:             "get_transaction",
			queueLimit:           cfg.RequestBacklogGetTransactionQueueLimit,
			requestDurationLimit: cfg.MaxGetTransactionExecutionDuration,
		},
		{
			methodName:           "sendTransaction",
			underlyingHandler:    methods.NewSendTransactionHandler(params.Daemon, params.Logger, params.TransactionStore, cfg.NetworkPassphrase),
			longName:             "send_transaction",
			queueLimit:           cfg.RequestBacklogSendTransactionQueueLimit,
			requestDurationLimit: cfg.MaxSendTransactionExecutionDuration,
		},
		{
			methodName:           "simulateTransaction",
			underlyingHandler:    methods.NewSimulateTransactionHandler(params.Logger, params.LedgerEntryReader, params.LedgerReader, params.PreflightGetter),
			longName:             "simulate_transaction",
			queueLimit:           cfg.RequestBacklogSimulateTransactionQueueLimit,
			requestDurationLimit: cfg.MaxSimulateTransactionExecutionDuration,
		},
	}
	handlersMap := handler.Map{}
	for _, handler := range handlers {
		queueLimiterGaugeName := handler.longName + "_inflight_requests"
		queueLimiterGaugeHelp := "Number of concurrenty in-flight " + handler.methodName + " requests"

		queueLimiterGauge := prometheus.NewGauge(prometheus.GaugeOpts{
			Namespace: params.Daemon.MetricsNamespace(), Subsystem: "network",
			Name: queueLimiterGaugeName,
			Help: queueLimiterGaugeHelp,
		})
		queueLimiter := network.MakeJrpcBacklogQueueLimiter(
			handler.underlyingHandler,
			queueLimiterGauge,
			uint64(handler.queueLimit),
			params.Logger)

		durationWarnCounterName := handler.longName + "_execution_threshold_warning"
		durationLimitCounterName := handler.longName + "_execution_threshold_limit"
		durationWarnCounterHelp := "The metric measures the count of " + handler.methodName + " requests that surpassed the warning threshold for execution time"
		durationLimitCounterHelp := "The metric measures the count of " + handler.methodName + " requests that surpassed the limit threshold for execution time"

		requestDurationWarnCounter := prometheus.NewCounter(prometheus.CounterOpts{
			Namespace: params.Daemon.MetricsNamespace(), Subsystem: "network",
			Name: durationWarnCounterName,
			Help: durationWarnCounterHelp,
		})
		requestDurationLimitCounter := prometheus.NewCounter(prometheus.CounterOpts{
			Namespace: params.Daemon.MetricsNamespace(), Subsystem: "network",
			Name: durationLimitCounterName,
			Help: durationLimitCounterHelp,
		})
		// set the warning threshold to be one third of the limit.
		requestDurationWarn := handler.requestDurationLimit / 3
		durationLimiter := network.MakeJrpcRequestDurationLimiter(
			queueLimiter.Handle,
			requestDurationWarn,
			handler.requestDurationLimit,
			requestDurationWarnCounter,
			requestDurationLimitCounter,
			params.Logger)
		handlersMap[handler.methodName] = durationLimiter.Handle
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

	queueLimitedBridge := network.MakeHTTPBacklogQueueLimiter(
		bridge,
		globalQueueRequestBacklogLimiter,
		uint64(cfg.RequestBacklogGlobalQueueLimit),
		params.Logger)

	globalQueueRequestExecutionDurationWarningCounter := prometheus.NewCounter(prometheus.CounterOpts{
		Namespace: params.Daemon.MetricsNamespace(), Subsystem: "network", Name: "global_request_execution_duration_threshold_warning",
		Help: "The metric measures the count of requests that surpassed the warning threshold for execution time",
	})
	globalQueueRequestExecutionDurationLimitCounter := prometheus.NewCounter(prometheus.CounterOpts{
		Namespace: params.Daemon.MetricsNamespace(), Subsystem: "network", Name: "global_request_execution_duration_threshold_limit",
		Help: "The metric measures the count of requests that surpassed the limit threshold for execution time",
	})
	var handler http.Handler = network.MakeHTTPRequestDurationLimiter(
		queueLimitedBridge,
		cfg.RequestExecutionWarningThreshold,
		cfg.MaxRequestExecutionDuration,
		globalQueueRequestExecutionDurationWarningCounter,
		globalQueueRequestExecutionDurationLimitCounter,
		params.Logger)

	handler = http.MaxBytesHandler(handler, maxHTTPRequestSize)

	corsMiddleware := cors.New(cors.Options{
		AllowedOrigins:         []string{},
		AllowOriginRequestFunc: func(*http.Request, string) bool { return true },
		AllowedHeaders:         []string{"*"},
		AllowedMethods:         []string{"GET", "PUT", "POST", "PATCH", "DELETE", "HEAD", "OPTIONS"},
	})

	return Handler{
		bridge:  bridge,
		logger:  params.Logger,
		Handler: corsMiddleware.Handler(handler),
	}
}
