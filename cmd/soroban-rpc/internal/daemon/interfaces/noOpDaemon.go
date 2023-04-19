package interfaces

import (
	"github.com/prometheus/client_golang/prometheus"
)

// The noOpDeamon is a dummy daemon implementation, supporting the Daemon interface.
// Used only in testing.
type noOpDaemon struct {
	metricsRegistry  *prometheus.Registry
	metricsNamespace string
}

func MakeNoOpDeamon() *noOpDaemon {
	return &noOpDaemon{
		metricsRegistry:  prometheus.NewRegistry(),
		metricsNamespace: "soroban_rpc",
	}
}

func (d *noOpDaemon) MetricsRegistry() *prometheus.Registry {
	return d.metricsRegistry
}

func (d *noOpDaemon) MetricsNamespace() string {
	return d.metricsNamespace
}
