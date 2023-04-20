package daemon

import (
	"context"
	"runtime"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/clients/stellarcore"
	proto "github.com/stellar/go/protocols/stellarcore"
	"github.com/stellar/go/support/logmetrics"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
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

type CoreClientWithMetrics struct {
	stellarcore.Client
	submitMetric  *prometheus.SummaryVec
	opCountMetric *prometheus.SummaryVec
}

func newCoreClientWithMetrics(client stellarcore.Client, registry *prometheus.Registry) *CoreClientWithMetrics {
	submitMetric := prometheus.NewSummaryVec(prometheus.SummaryOpts{
		Namespace: prometheusNamespace, Subsystem: "txsub", Name: "submission_duration_seconds",
		Help: "submission durations to Stellar-Core, sliding window = 10m",
	}, []string{"status"})
	opCountMetric := prometheus.NewSummaryVec(prometheus.SummaryOpts{
		Namespace: prometheusNamespace, Subsystem: "txsub", Name: "operation_count",
		Help: "number of operations included in a transaction, sliding window = 10m",
	}, []string{"status"})
	registry.MustRegister(submitMetric, opCountMetric)

	return &CoreClientWithMetrics{
		Client:        client,
		submitMetric:  submitMetric,
		opCountMetric: opCountMetric,
	}
}

func (c *CoreClientWithMetrics) SubmitTransaction(ctx context.Context, envelopeBase64 string) (*proto.TXResponse, error) {
	var envelope xdr.TransactionEnvelope
	err := xdr.SafeUnmarshalBase64(envelopeBase64, &envelope)
	if err != nil {
		return nil, err
	}

	startTime := time.Now()
	response, err := c.Client.SubmitTransaction(ctx, envelopeBase64)
	duration := time.Since(startTime).Seconds()

	var label prometheus.Labels
	if err != nil {
		label = prometheus.Labels{"status": "request_error"}
	} else if response.IsException() {
		label = prometheus.Labels{"status": "exception"}
	} else {
		label = prometheus.Labels{"status": response.Status}
	}

	c.submitMetric.With(label).Observe(duration)
	c.opCountMetric.With(label).Observe(float64(len(envelope.Operations())))
	return response, err
}

func (d *Daemon) CoreClient() interfaces.CoreClient {
	return d.coreClient
}
