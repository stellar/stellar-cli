package test

import (
	"fmt"
	"io"
	"net/http"
	"net/url"
	"runtime"
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
)

func TestMetrics(t *testing.T) {
	test := NewTest(t)
	metrics := getMetrics(test)
	buildMetric := fmt.Sprintf(
		"soroban_rpc_build_info{branch=\"%s\",build_timestamp=\"%s\",commit=\"%s\",goversion=\"%s\",version=\"%s\"} 1",
		config.Branch,
		config.BuildTimestamp,
		config.CommitHash,
		runtime.Version(),
		config.Version,
	)
	require.Contains(t, metrics, buildMetric)
}

func getMetrics(test *Test) string {
	metricsURL, err := url.JoinPath(test.adminURL(), "/metrics")
	require.NoError(test.t, err)
	response, err := http.Get(metricsURL)
	require.NoError(test.t, err)
	responseBytes, err := io.ReadAll(response.Body)
	require.NoError(test.t, err)
	require.NoError(test.t, response.Body.Close())
	return string(responseBytes)
}
