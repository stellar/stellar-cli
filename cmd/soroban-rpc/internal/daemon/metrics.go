package daemon

import (
	"runtime"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/support/logmetrics"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
)

func (d *Daemon) registerMetrics(registry *prometheus.Registry) {
	registry.MustRegister(prometheus.NewGoCollector())
	registry.MustRegister(prometheus.NewProcessCollector(prometheus.ProcessCollectorOpts{}))

	buildInfoGauge := prometheus.NewGaugeVec(
		prometheus.GaugeOpts{Namespace: "soroban_rpc", Subsystem: "build", Name: "info"},
		[]string{"version", "goversion", "commit", "branch", "build_timestamp"},
	)
	registry.MustRegister(buildInfoGauge)
	buildInfoGauge.With(prometheus.Labels{
		"version":         config.Version,
		"commit":          config.CommitHash,
		"branch":          config.Branch,
		"build_timestamp": config.BuildTimestamp,
		"goversion":       runtime.Version(),
	}).Inc()

	logMetrics := logmetrics.New("soroban_rpc")
	d.logger.AddHook(logMetrics)
	for _, counter := range logMetrics {
		registry.MustRegister(counter)
	}
}
