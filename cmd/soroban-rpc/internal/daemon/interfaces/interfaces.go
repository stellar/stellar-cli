package interfaces

import (
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/metrics"
)

type Daemon interface {
	MetricsRegistry() *metrics.Registry
}
