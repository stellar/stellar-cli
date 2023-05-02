package config

import (
	"fmt"
	"runtime"
	"testing"

	"github.com/sirupsen/logrus"
	"github.com/spf13/viper"
	"github.com/stellar/go/network"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func ExampleConfig() {
	var cfg Config
	var err error

	// If you want to load from cli flags, you must call Bind
	cfg.Bind()

	// Load values from: defaults, env vars, cli flags, then config file
	// Priority: defaults < config file < env vars < cli flags
	err = cfg.SetValues()
	if err != nil {
		panic(err)
	}

	// Ensure that what we parsed makes sense
	err = cfg.Validate()
	if err != nil {
		// This is commented in this example, because the default values are
		// missing some required fields, so the default config we parsed above is
		// invalid.
		// panic(err)
	}

	// Use the values
	fmt.Println(cfg.Endpoint)

	// Output: localhost:8000
}

func TestConfigLoadDefaults(t *testing.T) {
	// Set up a default config
	cfg := Config{}
	require.NoError(t, cfg.loadDefaults())

	// Check that the defaults are set
	assert.Equal(t, network.FutureNetworkPassphrase, cfg.NetworkPassphrase)
	assert.Equal(t, uint(runtime.NumCPU()), cfg.PreflightWorkerCount)
	// TODO: Check other defaults
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
