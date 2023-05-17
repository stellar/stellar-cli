package config

import (
	"runtime"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/spf13/viper"
	"github.com/stellar/go/network"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestLoadConfigPath(t *testing.T) {
	var cfg Config

	viper.Set("config-path", "./test.soroban.rpc.config")
	viper.Set("stellar-core-binary-path", "/usr/overriden/stellar-core")
	defer viper.Reset()

	require.NoError(t, cfg.SetValues())
	require.NoError(t, cfg.Validate())

	assert.Equal(t, cfg.CaptiveCoreConfigPath, "/opt/stellar/soroban-rpc/etc/stellar-captive-core.cfg")
	assert.Equal(t, cfg.StellarCoreBinaryPath, "/usr/bin/stellar-core", "env or cli flags should override --config-path values")
}

func TestConfigLoadDefaults(t *testing.T) {
	// Set up a default config
	cfg := Config{}
	require.NoError(t, cfg.loadDefaults())

	// Check that the defaults are set
	assert.Equal(t, network.FutureNetworkPassphrase, cfg.NetworkPassphrase)
	assert.Equal(t, uint(runtime.NumCPU()), cfg.PreflightWorkerCount)
}

func TestConfigLoadFlagsDefaultValuesOverrideExisting(t *testing.T) {
	// Set up a config with an existing non-default value
	cfg := Config{
		NetworkPassphrase: "existing value",
		LogLevel:          logrus.InfoLevel,
		Endpoint:          "localhost:8000",
	}

	// Set up a flag set with the default value
	viper.Set("network-passphrase", "")
	viper.Set("log-level", logrus.PanicLevel)
	defer viper.Reset()

	// Load the flags
	require.NoError(t, cfg.loadFlags())

	// Check that the flag value is set
	assert.Equal(t, "", cfg.NetworkPassphrase)
	assert.Equal(t, logrus.PanicLevel, cfg.LogLevel)

	// Check it didn't overwrite values which were not set in the flags
	assert.Equal(t, "localhost:8000", cfg.Endpoint)
}
