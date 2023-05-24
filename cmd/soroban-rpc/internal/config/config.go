package config

import (
	"os"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"

	support "github.com/stellar/go/support/config"
)

// Config represents the configuration of a soroban-rpc server
type Config struct {
	ConfigPath string

	Strict bool

	StellarCoreURL         string
	CaptiveCoreUseDB       bool
	CaptiveCoreStoragePath string
	StellarCoreBinaryPath  string
	CaptiveCoreConfigPath  string
	CaptiveCoreHTTPPort    uint

	Endpoint                         string
	AdminEndpoint                    string
	CheckpointFrequency              uint32
	CoreRequestTimeout               time.Duration
	DefaultEventsLimit               uint
	EventLedgerRetentionWindow       uint32
	FriendbotURL                     string
	HistoryArchiveURLs               []string
	IngestionTimeout                 time.Duration
	LogFormat                        LogFormat
	LogLevel                         logrus.Level
	MaxEventsLimit                   uint
	MaxHealthyLedgerLatency          time.Duration
	NetworkPassphrase                string
	PreflightWorkerCount             uint
	PreflightWorkerQueueSize         uint
	SQLiteDBPath                     string
	TransactionLedgerRetentionWindow uint32

	// We memoize these, so they bind to viper flags correctly
	optionsCache *ConfigOptions
	flagsCache   *support.ConfigOptions
	viper        *viper.Viper
}

func (cfg *Config) Init(cmd *cobra.Command) error {
	return cfg.flags().Init(cmd)
}

// We start with the defaults
func (cfg *Config) SetValues() error {
	if err := cfg.loadDefaults(); err != nil {
		return err
	}

	// Then we load from the cli flags and environment variables
	if err := cfg.loadFlags(); err != nil {
		return err
	}

	// If we specified a config file, we load that
	if cfg.ConfigPath != "" {
		// Merge in the config file flags
		if err := cfg.loadConfigPath(); err != nil {
			return err
		}

		// Load from cli flags and environment variables again, to overwrite what we
		// got from the config file
		if err := cfg.loadFlags(); err != nil {
			return err
		}
	}

	return nil
}

// loadDefaults populates the config with default values
func (cfg *Config) loadDefaults() error {
	for _, option := range cfg.options() {
		if option.DefaultValue != nil {
			if err := option.setValue(option.DefaultValue); err != nil {
				return err
			}
		}
	}
	return nil
}

// loadFlags populates the config with values from the cli flags and
// environment variables
func (cfg *Config) loadFlags() error {
	cfg.Bind()
	for _, option := range cfg.options() {
		if cfg.viper.IsSet(option.Name) {
			if err := option.setValue(cfg.viper.Get(option.Name)); err != nil {
				return err
			}
		}
	}
	return nil
}

// loadConfigPath loads a new config from a toml file at the given path. Strict
// mode will return an error if there are any unknown toml variables set. Note,
// strict-mode can also be set by putting `STRICT=true` in the config.toml file
// itself.
func (cfg *Config) loadConfigPath() error {
	file, err := os.Open(cfg.ConfigPath)
	if err != nil {
		return err
	}
	defer file.Close()
	return parseToml(file, cfg.Strict, cfg)
}

func (cfg *Config) Validate() error {
	return cfg.options().Validate()
}
