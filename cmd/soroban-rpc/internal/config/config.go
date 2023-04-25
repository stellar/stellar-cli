package config

import (
	"os"
	"reflect"

	"github.com/sirupsen/logrus"
	"github.com/spf13/cobra"

	"github.com/stellar/go/support/errors"
)

// Config represents the configuration of a friendbot server
type Config struct {
	// Optional: The path to the config file. Not in the toml, as wouldn't make sense.
	ConfigPath string

	// TODO: Enforce this when parsing this toml file
	Strict bool

	StellarCoreURL         string
	CaptiveCoreUseDB       bool
	CaptiveCoreStoragePath string
	StellarCoreBinaryPath  string
	CaptiveCoreConfigPath  string
	CaptiveCoreHTTPPort    int

	Endpoint                         string
	AdminEndpoint                    string
	CheckpointFrequency              uint32
	CoreRequestTimeout               Duration
	DefaultEventsLimit               uint
	EventLedgerRetentionWindow       PositiveUint32
	FriendbotURL                     string
	HistoryArchiveURLs               []string
	IngestionTimeout                 Duration
	LogFormat                        LogFormat
	LogLevel                         logrus.Level
	MaxEventsLimit                   uint
	MaxHealthyLedgerLatency          Duration
	NetworkPassphrase                string
	PreflightWorkerCount             PositiveUint
	PreflightWorkerQueueSize         PositiveUint
	SQLiteDBPath                     string
	TransactionLedgerRetentionWindow PositiveUint32
}

func (cfg *Config) Init(cmd *cobra.Command) error {
	return cfg.flags().Init(cmd)
}

func (cfg *Config) SetValues() error {
	// We start with the defaults
	if err := cfg.SetDefaults(); err != nil {
		return err
	}

	// Then we load from the flags
	flags := Config{}
	err := flags.flags().SetValues()
	if err != nil {
		return err
	}

	// Merge flags on top of the defaults
	*cfg = cfg.Merge(flags)

	// If we specified a config file, we load that but give CLI-flags precedence
	if cfg.ConfigPath != "" {
		fileConfig, err := Read(cfg.ConfigPath, cfg.Strict)
		if err != nil {
			return errors.Wrap(err, "reading config file")
		}
		*cfg = fileConfig.Merge(*cfg)
	}

	// Finally, we can validate the config
	return cfg.Validate()
}

func (cfg *Config) SetDefaults() error {
	for _, option := range cfg.options() {
		if option.DefaultValue != nil {
			option.setValue(option.DefaultValue)
		}
	}
	return nil
}

func Read(path string, strict bool) (*Config, error) {
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
// TODO: Unit-test this
func mergeStructs(a, b reflect.Value) reflect.Value {
	if a.Type() != b.Type() {
		panic("Cannot merge structs of different types")
	}
	structType := a.Type()
	merged := reflect.New(structType).Elem()
	for i := 0; i < structType.NumField(); i++ {
		if !merged.Field(i).CanSet() {
			// Can't set unexported fields
			// TODO: Figure out how to fix this, cause this means it can't set the captiveCoreTomlValues
			continue
		}
		val := b.Field(i)
		if val.IsZero() {
			val = a.Field(i)
		}
		if val.Kind() == reflect.Struct {
			// Recurse into structs
			val = mergeStructs(a.Field(i), b.Field(i))
		}
		merged.Field(i).Set(val)

	}
	return merged
}

func (cfg Config) Merge(cfg2 Config) Config {
	return mergeStructs(
		reflect.ValueOf(cfg),
		reflect.ValueOf(cfg2),
	).Interface().(Config)
}
