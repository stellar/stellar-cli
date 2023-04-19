package daemon

import (
	"runtime"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/support/logmetrics"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
)

func (d *Daemon) registerMetrics() {
	buildInfoGauge := prometheus.NewGaugeVec(
		prometheus.GaugeOpts{Namespace: prometheusNamespace, Subsystem: "build", Name: "info"},
		[]string{"version", "goversion", "commit", "branch", "build_timestamp"},
	)
	// LogMetricsHook is a metric which counts log lines emitted by soroban rpc
	LogMetricsHook := logmetrics.New(prometheusNamespace)
	//
	buildInfoGauge.With(prometheus.Labels{
		"version":         config.Version,
		"commit":          config.CommitHash,
		"branch":          config.Branch,
		"build_timestamp": config.BuildTimestamp,
		"goversion":       runtime.Version(),
	}).Inc()

	d.metricsRegistry.MustRegister(prometheus.NewGoCollector())
	d.metricsRegistry.MustRegister(prometheus.NewProcessCollector(prometheus.ProcessCollectorOpts{}))
	d.metricsRegistry.MustRegister(buildInfoGauge)

	for _, counter := range LogMetricsHook {
		d.metricsRegistry.MustRegister(counter)
	}
}

func (d *Daemon) MetricsRegistry() *prometheus.Registry {
	return d.metricsRegistry
}

func (d *Daemon) MetricsNamespace() string {
	return prometheusNamespace
}
