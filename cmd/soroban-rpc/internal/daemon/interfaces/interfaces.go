package interfaces

import (
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/metrics"
)

// Daemon defines the interface that the Daemon would be implementing.
// this wuold be useful for decoupling purposes, allowing to test components without
// the actual daemon.
type Daemon interface {
	MetricsRegistry() *metrics.Registry
}
