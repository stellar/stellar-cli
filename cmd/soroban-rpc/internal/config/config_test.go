package config

import (
	"runtime"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/stellar/go/network"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestLoadConfigPath(t *testing.T) {
	var cfg Config

	cfg.Bind()
	cfg.viper.Set("config-path", "./test.soroban.rpc.config")
	cfg.viper.Set("stellar-core-binary-path", "/usr/overridden/stellar-core")
	cfg.viper.Set("network-passphrase", "CLI test passphrase")

	require.NoError(t, cfg.SetValues())
	require.NoError(t, cfg.Validate())

	assert.Equal(t, "/opt/stellar/soroban-rpc/etc/stellar-captive-core.cfg", cfg.CaptiveCoreConfigPath)
	assert.Equal(t, "/usr/overridden/stellar-core", cfg.StellarCoreBinaryPath, "env or cli flags should override --config-path values")
	assert.Equal(t, "CLI test passphrase", cfg.NetworkPassphrase, "env or cli flags should override --config-path values")
	assert.Equal(t, "/opt/stellar/soroban-rpc/rpc_db.sqlite", cfg.SQLiteDBPath, "config file should fill in if not set on the cli or env")
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
	cfg.Bind()

	// Set up a flag set with the default value
	cfg.viper.Set("network-passphrase", "")
	cfg.viper.Set("log-level", logrus.PanicLevel)

	// Load the flags
	require.NoError(t, cfg.loadFlags())

	// Check that the flag value is set
	assert.Equal(t, "", cfg.NetworkPassphrase)
	assert.Equal(t, logrus.PanicLevel, cfg.LogLevel)

	// Check it didn't overwrite values which were not set in the flags
	assert.Equal(t, "localhost:8000", cfg.Endpoint)
}
