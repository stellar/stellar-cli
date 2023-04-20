package interfaces

import (
	"context"

	"github.com/prometheus/client_golang/prometheus"
	proto "github.com/stellar/go/protocols/stellarcore"
)

// The noOpDeamon is a dummy daemon implementation, supporting the Daemon interface.
// Used only in testing.
type noOpDaemon struct {
	metricsRegistry  *prometheus.Registry
	metricsNamespace string
	coreClient       noOpCoreClient
}

func MakeNoOpDeamon() *noOpDaemon {
	return &noOpDaemon{
		metricsRegistry:  prometheus.NewRegistry(),
		metricsNamespace: "soroban_rpc",
		coreClient:       noOpCoreClient{},
	}
}

func (d *noOpDaemon) MetricsRegistry() *prometheus.Registry {
	return d.metricsRegistry
}

func (d *noOpDaemon) MetricsNamespace() string {
	return d.metricsNamespace
}

func (d *noOpDaemon) CoreClient() CoreClient {
	return d.coreClient
}

type noOpCoreClient struct{}

func (s noOpCoreClient) Info(context.Context) (*proto.InfoResponse, error) {
	return &proto.InfoResponse{}, nil
}

func (s noOpCoreClient) SubmitTransaction(context.Context, string) (*proto.TXResponse, error) {
	return &proto.TXResponse{Status: proto.PreflightStatusOk}, nil
}
