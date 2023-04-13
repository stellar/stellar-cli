package test

import (
	"fmt"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"io"
	"net/http"
	"runtime"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestMetrics(t *testing.T) {
	test := NewTest(t)
	response, err := http.Get(test.adminURL() + "/metrics")
	require.NoError(t, err)
	responseBytes, err := io.ReadAll(response.Body)
	require.NoError(t, err)
	require.NoError(t, response.Body.Close())
	responseString := string(responseBytes)
	t.Log(responseString)
	buildMetric := fmt.Sprintf(
		"soroban_rpc_build_info{branch=\"%s\",build_timestamp=\"%s\",commit=\"%s\",goversion=\"%s\",version=\"%s\"} 1",
		config.Branch,
		config.BuildTimestamp,
		config.CommitHash,
		runtime.Version(),
		config.Version,
	)
	require.Contains(t, responseString, buildMetric)
}
