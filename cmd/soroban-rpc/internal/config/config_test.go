package config

import (
	"fmt"
	"runtime"
	"testing"

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
	a := Config{
		NetworkPassphrase: "only in a",
		FriendbotURL:      "in both (a)",
	}
	b := Config{
		Endpoint:     "only in b",
		FriendbotURL: "in both (b)",
	}
	c := a.Merge(b)

	// Values only in a should be preserved
	assert.Equal(t, a.NetworkPassphrase, c.NetworkPassphrase)

	// Values only in b should be preserved
	assert.Equal(t, b.Endpoint, c.Endpoint)

	// Values in b should take precedence over values in a
	assert.Equal(t, b.FriendbotURL, c.FriendbotURL)

	// Check that the original configs are unchanged
	assert.Equal(t, "only in a", a.NetworkPassphrase)
	assert.Equal(t, "only in b", b.Endpoint)
	assert.Equal(t, "in both (a)", a.FriendbotURL)
	assert.Equal(t, "in both (b)", b.FriendbotURL)
}
