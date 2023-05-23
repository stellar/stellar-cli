package config

import (
	"fmt"
	"go/types"
	"os"
	"strconv"
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
		// the viper package is not overly developed, so we're going to do some work-arounds to figure out
		// if a key was truely set or not so that we would read it only if it was truely set by the command
		// line. ( i.e. the viper package internally sets the "defaults" whenever a pflag is attached. )
		flag := option.flag()
		switch flag.OptType {
		case types.Bool:
			if setValue, ok := viper.Get(option.Name).(bool); ok {
				if (option.DefaultValue == nil && setValue == false) || setValue == option.DefaultValue.(bool) {
					// we need to determine if we received the default value or the actual user-set value.
					viper.SetDefault(option.Name, !setValue)
					if viper.Get(option.Name).(bool) == setValue {
						// the user truely specified a value
						option.setValue(setValue)
					}
					// restore the viper default value.
					viper.SetDefault(option.Name, option.DefaultValue)
				} else {
					// the value is differ then the default, therefore, it must be a user-specified value
					option.setValue(setValue)
				}
			}
		case types.Uint32:
		case types.Uint:
			if stringSetValue, ok := viper.Get(option.Name).(string); ok {
				setValue, err := strconv.ParseUint(stringSetValue, 10, 64)
				if err != nil {
					return err
				}
				if (option.DefaultValue == nil && setValue == 0) || uint(setValue) == option.DefaultValue.(uint) {
					// we need to determine if we received the default value or the actual user-set value.
					viper.SetDefault(option.Name, ^setValue)
					if viper.Get(option.Name).(uint64) == uint64(setValue) {
						// the user truely specified a value
						option.setValue(setValue)
					}
					// restore the viper default value.
					viper.SetDefault(option.Name, option.DefaultValue)
				} else {
					// the value is differ then the default, therefore, it must be a user-specified value
					option.setValue(setValue)
				}
			}
		case types.String:
			val := viper.Get(option.Name)
			if setValue, ok := val.(string); ok {
				if (option.DefaultValue == nil && setValue == "") || setValue == fmt.Sprintf("%s", option.DefaultValue) {
					// we need to determine if we received the default value or the actual user-set value.
					viper.SetDefault(option.Name, setValue+"!")
					if viper.Get(option.Name).(string) == setValue {
						// the user truely specified a value
						option.setValue(setValue)
					}
					// restore the viper default value.
					viper.SetDefault(option.Name, option.DefaultValue)
				} else {
					// the value is differ then the default, therefore, it must be a user-specified value
					option.setValue(setValue)
				}
			} else {
				// this could be some custom data type.
				option.setValue(setValue)
			}
		default:
			return fmt.Errorf("unsupported option type %v in loadFlags", flag.OptType)
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
