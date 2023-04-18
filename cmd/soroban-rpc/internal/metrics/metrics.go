package metrics

import (
	"net/http"
	"runtime"

	"github.com/prometheus/client_golang/prometheus/promhttp"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/support/logmetrics"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
)

const (
	prometheusNamespace = "soroban_rpc"
)

type PrometheusRegistry = *prometheus.Registry
type SummaryVec = prometheus.SummaryVec
type Gauge = prometheus.Gauge
type CounterVec = prometheus.CounterVec

// extend the prometheus registry by adding the http handler.
type Registry struct {
	PrometheusRegistry
	HTTPHandler http.Handler
}

func MakeRegistry() *Registry {
	registry := prometheus.NewRegistry()
	buildInfoGauge := prometheus.NewGaugeVec(
		prometheus.GaugeOpts{Namespace: prometheusNamespace, Subsystem: "build", Name: "info"},
		[]string{"version", "goversion", "commit", "branch", "build_timestamp"},
	)
	// LogMetricsHook is a metric which counts log lines emitted by soroban rpc
	LogMetricsHook := logmetrics.New(prometheusNamespace)

	// HTTPHandler is prometheus HTTP handler for sorban rpc metrics
	httpHandler := promhttp.HandlerFor(registry, promhttp.HandlerOpts{})

	buildInfoGauge.With(prometheus.Labels{
		"version":         config.Version,
		"commit":          config.CommitHash,
		"branch":          config.Branch,
		"build_timestamp": config.BuildTimestamp,
		"goversion":       runtime.Version(),
	}).Inc()

	registry.MustRegister(prometheus.NewGoCollector())
	registry.MustRegister(prometheus.NewProcessCollector(prometheus.ProcessCollectorOpts{}))
	registry.MustRegister(buildInfoGauge)

	for _, counter := range LogMetricsHook {
		registry.MustRegister(counter)
	}

	return &Registry{registry, httpHandler}
}

func (r *Registry) Namespace() string {
	return prometheusNamespace
}
