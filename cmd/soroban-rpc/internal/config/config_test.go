package config

import (
	"runtime"
	"testing"

	"github.com/stellar/go/network"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestConfigSetDefaults(t *testing.T) {
	// Set up a default config
	cfg := Config{}
	require.NoError(t, cfg.SetDefaults())

	// Check that the defaults are set
	assert.Equal(t, network.FutureNetworkPassphrase, cfg.NetworkPassphrase)
	assert.Equal(t, uint(runtime.NumCPU()), cfg.PreflightWorkerCount)
	// TODO: Check other defaults
}

func TestMerge(t *testing.T) {
	t.Error("TODO: Implement")
}
