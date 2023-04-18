package interfaces

import "github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/metrics"

// The noOpDeamon is a dummy daemon implementation, supporting the Daemon interface.
// Used only in testing.
type noOpDaemon struct {
	metricsRegistry *metrics.Registry
}

func MakeNoOpDeamon() *noOpDaemon {
	return &noOpDaemon{
		metricsRegistry: metrics.MakeNoOpRegistry(),
	}
}

func (d *noOpDaemon) MetricsRegistry() *metrics.Registry {
	return d.metricsRegistry
}
