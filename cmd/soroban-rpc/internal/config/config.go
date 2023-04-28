package config

import (
	"os"
	"reflect"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/spf13/cobra"

	support "github.com/stellar/go/support/config"
	"github.com/stellar/go/support/errors"
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
		fileConfig, err := loadConfigPath(cfg.ConfigPath, cfg.Strict)
		if err != nil {
			return errors.Wrap(err, "reading config file")
		}
		// Merge in the config file flags, giving CLI-flags precedence
		*cfg = fileConfig.Merge(*cfg)
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
	flags := Config{}
	err := flags.flags().SetValues()
	if err != nil {
		return err
	}

	// Merge flags on top of the defaults
	*cfg = cfg.Merge(flags)
	return nil
}

// loadConfigPath loads a new config from a toml file at the given path. Strict
// mode will return an error if there are any unknown toml variables set. Note,
// strict-mode can also be set by putting `STRICT=true` in the config.toml file
// itself.
func loadConfigPath(path string, strict bool) (*Config, error) {
	cfg := &Config{}

	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	err = parseToml(file, strict, cfg)
	if err != nil {
		return nil, err
	}
	return cfg, nil
}

func (cfg *Config) Validate() error {
	return cfg.options().Validate()
}

// Merge a and b, preferring values from b. Neither config is modified, instead
// a new struct is returned.
func (cfg Config) Merge(cfg2 Config) Config {
	a := reflect.ValueOf(cfg)
	b := reflect.ValueOf(cfg2)
	structType := a.Type()
	merged := reflect.New(structType).Elem()
	for i := 0; i < structType.NumField(); i++ {
		if !merged.Field(i).CanSet() {
			// Can't set unexported fields
			continue
		}
		val := b.Field(i)
		if val.IsZero() {
			val = a.Field(i)
		}
		merged.Field(i).Set(val)

	}
	return merged.Interface().(Config)
}
