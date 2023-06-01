package config

import (
	"runtime"
	"testing"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/stellar/go/network"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestLoadConfigPathPrecedence(t *testing.T) {
	var cfg Config

	cmd := &cobra.Command{}
	require.NoError(t, cfg.AddFlags(cmd))
	require.NoError(t, cmd.ParseFlags([]string{
		"--config-path", "./test.soroban.rpc.config",
		"--stellar-core-binary-path", "/usr/overridden/stellar-core",
		"--network-passphrase", "CLI test passphrase",
	}))

	require.NoError(t, cfg.SetValues(func(key string) (string, bool) {
		switch key {
		case "STELLAR_CORE_BINARY_PATH":
			return "/env/stellar-core", true
		case "DB_PATH":
			return "/env/overridden/db", true
		default:
			return "", false
		}
	}))
	require.NoError(t, cfg.Validate())

	assert.Equal(t, "/opt/stellar/soroban-rpc/etc/stellar-captive-core.cfg", cfg.CaptiveCoreConfigPath, "should read values from the config path file")
	assert.Equal(t, "CLI test passphrase", cfg.NetworkPassphrase, "cli flags should override --config-path values")
	assert.Equal(t, "/usr/overridden/stellar-core", cfg.StellarCoreBinaryPath, "cli flags should override --config-path values and env vars")
	assert.Equal(t, "/env/overridden/db", cfg.SQLiteDBPath, "env var should override config file")
	assert.Equal(t, 2*time.Second, cfg.CoreRequestTimeout, "default value should be used, if not set anywhere else")
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

	cmd := &cobra.Command{}
	require.NoError(t, cfg.AddFlags(cmd))
	// Set up a flag set with the default value
	require.NoError(t, cmd.ParseFlags([]string{
		"--network-passphrase", "",
		"--log-level", logrus.PanicLevel.String(),
	}))

	// Load the flags
	require.NoError(t, cfg.loadFlags())

	// Check that the flag value is set
	assert.Equal(t, "", cfg.NetworkPassphrase)
	assert.Equal(t, logrus.PanicLevel, cfg.LogLevel)

	// Check it didn't overwrite values which were not set in the flags
	assert.Equal(t, "localhost:8000", cfg.Endpoint)
}
