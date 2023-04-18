package metrics

import (
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"runtime"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/support/logmetrics"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
)

const (
	prometheusNamespace = "soroban_rpc"
)

var (
	// Registry is the prometheuse registry for soroban rpc metrics
	Registry       = prometheus.NewRegistry()
	buildInfoGauge = prometheus.NewGaugeVec(
		prometheus.GaugeOpts{Namespace: "soroban_rpc", Subsystem: "build", Name: "info"},
		[]string{"version", "goversion", "commit", "branch", "build_timestamp"},
	)
	// LogMetricsHook is a metric which counts log lines emitted by soroban rpc
	LogMetricsHook = logmetrics.New(prometheusNamespace)
	// HTTPHandler is prometheus HTTP handler for sorban rpc metrics
	HTTPHandler = promhttp.HandlerFor(Registry, promhttp.HandlerOpts{})

	// EventsDurationMetric is a metric for measuring latency of event store operations
	EventsDurationMetric = prometheus.NewSummaryVec(prometheus.SummaryOpts{
		Namespace: prometheusNamespace, Subsystem: "events", Name: "operation_duration_seconds",
		Help: "event store operation durations, sliding window = 10m",
	},
		[]string{"operation"},
	)
	// TransactionDurationMetric is a metric for measuring latency of transaction store operations
	TransactionDurationMetric = prometheus.NewSummaryVec(prometheus.SummaryOpts{
		Namespace: prometheusNamespace, Subsystem: "transactions", Name: "operation_duration_seconds",
		Help: "transaction store operation durations, sliding window = 10m",
	},
		[]string{"operation"},
	)
	// LatestLedgerMetric is a metric for measuring the latest ingested ledger
	LatestLedgerMetric = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: prometheusNamespace, Subsystem: "ingest", Name: "local_latest_ledger",
		Help: "sequence number of the latest ledger ingested by this ingesting instance",
	})
	// IngestionDurationMetric is a metric for measuring the latency of ingestion
	IngestionDurationMetric = prometheus.NewSummaryVec(prometheus.SummaryOpts{
		Namespace: prometheusNamespace, Subsystem: "ingest", Name: "ledger_ingestion_duration_seconds",
		Help: "ledger ingestion durations, sliding window = 10m",
	},
		[]string{"type"},
	)
	// LedgerStatsMetric is a metric which measures statistics on all ledger entries ingested by soroban rpc
	LedgerStatsMetric = prometheus.NewCounterVec(
		prometheus.CounterOpts{
			Namespace: prometheusNamespace, Subsystem: "ingest", Name: "ledger_stats_total",
			Help: "counters of different ledger stats",
		},
		[]string{"type"},
	)
)

func init() {
	buildInfoGauge.With(prometheus.Labels{
		"version":         config.Version,
		"commit":          config.CommitHash,
		"branch":          config.Branch,
		"build_timestamp": config.BuildTimestamp,
		"goversion":       runtime.Version(),
	}).Inc()

	Registry.MustRegister(prometheus.NewGoCollector())
	Registry.MustRegister(prometheus.NewProcessCollector(prometheus.ProcessCollectorOpts{}))
	Registry.MustRegister(EventsDurationMetric)
	Registry.MustRegister(TransactionDurationMetric)
	Registry.MustRegister(LatestLedgerMetric)
	Registry.MustRegister(IngestionDurationMetric)
	Registry.MustRegister(LedgerStatsMetric)

	for _, counter := range LogMetricsHook {
		Registry.MustRegister(counter)
	}
}
