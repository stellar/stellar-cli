package interfaces

import "github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/metrics"

type NoOpDeamon struct {
	metricsRegistry *metrics.Registry
}

func MakeNoOpDeamon() *NoOpDeamon {
	return &NoOpDeamon{
		metricsRegistry: metrics.MakeNoOpRegistry(),
	}
}

func (d *NoOpDeamon) MetricsRegistry() *metrics.Registry {
	return d.metricsRegistry
}
