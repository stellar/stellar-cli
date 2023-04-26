package config

import (
	"strings"
	"testing"

	"github.com/stellar/go/network"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

const basicToml = `
HISTORY_ARCHIVE_URLS = [ "http://history-futurenet.stellar.org" ]
NETWORK_PASSPHRASE = "Test SDF Future Network ; October 2022"

# TODO: Maybe this would make more sense as STELLAR_CORE.BINARY_PATH
STELLAR_CORE_BINARY_PATH = "/usr/bin/stellar-core"
CAPTIVE_CORE_USE_DB = true
CAPTIVE_CORE_STORAGE_PATH = "/etc/stellar/soroban-rpc"
CAPTIVE_CORE_CONFIG_PATH = "/etc/stellar/soroban-rpc/captive-core.cfg"
`

func TestBasicTomlReading(t *testing.T) {
	cfg := Config{}
	require.NoError(t, parseToml(strings.NewReader(basicToml), false, &cfg))

	// Check a few fields got read correctly
	assert.Equal(t, []string{"http://history-futurenet.stellar.org"}, cfg.HistoryArchiveURLs)
	assert.Equal(t, network.FutureNetworkPassphrase, cfg.NetworkPassphrase)
	assert.Equal(t, "/usr/bin/stellar-core", cfg.StellarCoreBinaryPath)
}

func TestBasicTomlReadingStrictMode(t *testing.T) {
	invalidToml := `UNKNOWN = "key"`
	cfg := Config{}

	// Should panic when unknown key and strict set in the cli flags
	require.EqualError(
		t,
		parseToml(strings.NewReader(invalidToml), true, &cfg),
		"Invalid config: unknown field \"UNKNOWN\"",
	)

	// Should panic when unknown key and strict set in the config file
	invalidStrictToml := `
	STRICT = true
	UNKNOWN = "key"
`
	require.EqualError(
		t,
		parseToml(strings.NewReader(invalidStrictToml), false, &cfg),
		"Invalid config: unknown field \"UNKNOWN\"",
	)

	// It passes on a valid config
	require.NoError(t, parseToml(strings.NewReader(basicToml), true, &cfg))
}

func TestBasicTomlWriting(t *testing.T) {
	// Set up a default config
	cfg := Config{}
	require.NoError(t, cfg.LoadDefaults())

	// Output it to toml
	outBytes, err := cfg.MarshalTOML()
	require.NoError(t, err)

	out := string(outBytes)

	// Spot-check that the output looks right. Try to check one value for each
	// type of option. (string, duration, uint, etc...)
	assert.Contains(t, out, "NETWORK_PASSPHRASE = \"Test SDF Future Network ; October 2022\"")
	assert.Contains(t, out, "STELLAR_CORE_TIMEOUT = \"2s\"")
	assert.Contains(t, out, "STELLAR_CAPTIVE_CORE_HTTP_PORT = 11626")
	assert.Contains(t, out, "LOG_LEVEL = \"info\"")
	assert.Contains(t, out, "LOG_FORMAT = \"text\"")

	// Check that the output contains comments about each option
	assert.Contains(t, out, "# Network passphrase of the Stellar network transactions should be signed for")
}
