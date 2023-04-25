package config

import (
	"bytes"
	"strings"
	"testing"

	"github.com/stellar/go/network"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

const basicToml = `
HISTORY_ARCHIVE_URLS = [ "http://history-futurenet.stellar.org" ]
NETWORK_PASSPHRASE = "Test SDF Future Network ; October 2022"

# TODO: This would make more sense as STELLAR_CORE.BINARY_PATH
STELLAR_CORE_BINARY_PATH = "/usr/bin/stellar-core"
CAPTIVE_CORE_USE_DB = true
CAPTIVE_CORE_STORAGE_PATH = "/etc/stellar/soroban-rpc"
CAPTIVE_CORE_CONFIG_PATH = "/etc/stellar/soroban-rpc/captive-core.cfg"
`

func TestBasicTomlReading(t *testing.T) {
	cfg := Config{}
	require.NoError(t, parseToml(strings.NewReader(basicToml), false, &cfg))
	require.NoError(t, cfg.Validate())

	// Check a few fields got read correctly
	assert.Equal(t, []string{"http://history-futurenet.stellar.org"}, cfg.HistoryArchiveURLs)
	assert.Equal(t, network.FutureNetworkPassphrase, cfg.NetworkPassphrase)
	assert.Equal(t, "/usr/bin/stellar-core", cfg.StellarCoreBinaryPath)
}

func TestBasicTomlWriting(t *testing.T) {
	// Set up a default config
	cfg := Config{}
	require.NoError(t, cfg.SetDefaults())

	// Output it to toml
	buf := &bytes.Buffer{}
	require.NoError(t, cfg.marshalToml(buf))

	// Spot-check that the output looks right
	t.Log(buf.String())
}
