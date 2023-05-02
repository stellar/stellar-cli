package config

import (
	"bytes"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/stellar/go/network"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

const basicToml = `
HISTORY_ARCHIVE_URLS = [ "http://history-futurenet.stellar.org" ]
NETWORK_PASSPHRASE = "Test SDF Future Network ; October 2022"

# testing comments work ok
STELLAR_CORE_BINARY_PATH = "/usr/bin/stellar-core"
CAPTIVE_CORE_USE_DB = true
CAPTIVE_CORE_STORAGE_PATH = "/etc/stellar/soroban-rpc"
CAPTIVE_CORE_CONFIG_PATH = "/etc/stellar/soroban-rpc/captive-core.cfg"
`

func TestBasicTomlReading(t *testing.T) {
	cfg := Config{}
	require.NoError(t, parseToml(strings.NewReader(basicToml), false, &cfg))

	// Check the fields got read correctly
	assert.Equal(t, []string{"http://history-futurenet.stellar.org"}, cfg.HistoryArchiveURLs)
	assert.Equal(t, network.FutureNetworkPassphrase, cfg.NetworkPassphrase)
	assert.Equal(t, true, cfg.CaptiveCoreUseDB)
	assert.Equal(t, "/etc/stellar/soroban-rpc", cfg.CaptiveCoreStoragePath)
	assert.Equal(t, "/etc/stellar/soroban-rpc/captive-core.cfg", cfg.CaptiveCoreConfigPath)
}

func TestBasicTomlReadingStrictMode(t *testing.T) {
	invalidToml := `UNKNOWN = "key"`
	cfg := Config{}

	// Should ignore unknown fields when strict is not set
	require.NoError(t, parseToml(strings.NewReader(invalidToml), false, &cfg))

	// Should panic when unknown key is present and strict is set in the cli
	// flags
	require.EqualError(
		t,
		parseToml(strings.NewReader(invalidToml), true, &cfg),
		"invalid config: unexpected entry specified in toml file \"UNKNOWN\"",
	)

	// Should panic when unknown key is present and strict is set in the
	// config file
	invalidStrictToml := `
	STRICT = true
	UNKNOWN = "key"
`
	require.EqualError(
		t,
		parseToml(strings.NewReader(invalidStrictToml), false, &cfg),
		"invalid config: unexpected entry specified in toml file \"UNKNOWN\"",
	)

	// It succeeds with a valid config
	require.NoError(t, parseToml(strings.NewReader(basicToml), true, &cfg))
}

func TestBasicTomlWriting(t *testing.T) {
	// Set up a default config
	cfg := Config{}
	require.NoError(t, cfg.loadDefaults())

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

	// Test that it wraps long lines.
	// Note the newline at char 80. This also checks it adds a space after the
	// comment when outputting multi-line comments, which go-toml does *not* do
	// by default.
	assert.Contains(t, out, "# configures the event retention window expressed in number of ledgers, the\n# default value is 17280 which corresponds to about 24 hours of history")
}

func TestRoundTrip(t *testing.T) {
	// Set up a default config
	cfg := Config{}
	require.NoError(t, cfg.loadDefaults())

	// Generate test values for every option, so we can round-trip test them all.
	for _, option := range cfg.options() {
		optType := reflect.ValueOf(option.ConfigKey).Elem().Type()
		switch option.ConfigKey.(type) {
		case *bool:
			*option.ConfigKey.(*bool) = true
		case *string:
			*option.ConfigKey.(*string) = "test"
		case *uint:
			*option.ConfigKey.(*uint) = 42
		case *uint32:
			*option.ConfigKey.(*uint32) = 32
		case *time.Duration:
			*option.ConfigKey.(*time.Duration) = 5 * time.Second
		case *[]string:
			*option.ConfigKey.(*[]string) = []string{"a", "b"}
		case *logrus.Level:
			*option.ConfigKey.(*logrus.Level) = logrus.InfoLevel
		case *LogFormat:
			*option.ConfigKey.(*LogFormat) = LogFormatText
		default:
			t.Fatalf("TestRoundTrip not implemented for type %s, on option %s, please add a test value", optType.Kind(), option.Name)
		}
	}

	// Output it to toml
	outBytes, err := cfg.MarshalTOML()
	require.NoError(t, err)

	// t.Log(string(outBytes))

	// Parse it back
	require.NoError(
		t,
		parseToml(bytes.NewReader(outBytes), false, &cfg),
	)
}
