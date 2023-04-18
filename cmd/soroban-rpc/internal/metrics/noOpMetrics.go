package metrics

import (
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promhttp"
)

func MakeNoOpRegistry() *Registry {
	registry := prometheus.NewRegistry()
	// HTTPHandler is prometheus HTTP handler for sorban rpc metrics
	httpHandler := promhttp.HandlerFor(registry, promhttp.HandlerOpts{})

	return &Registry{registry, httpHandler}
}
