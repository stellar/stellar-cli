package test

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestMetrics(t *testing.T) {
	test := NewTest(t)
	metrics, err := test.daemon.PrometheusRegistry().Gather()
	assert.NoError(t, err)
	for _, metricFamily := range metrics {
		if metricFamily.GetName() == "soroban_rpc_build_info" {
			metric := metricFamily.GetMetric()
			assert.Len(t, metric, 1)
			assert.Equal(t, float64(1), metric[0].GetGauge().GetValue())
			return
		}
	}
	t.Fatalf("could not find soroban_rpc_build_info metric")
}
