package config

import (
	"reflect"

	"github.com/sirupsen/logrus"
	"github.com/spf13/cobra"

	support "github.com/stellar/go/support/config"
	"github.com/stellar/go/support/errors"
)

// Config represents the configuration of a friendbot server
type Config struct {
	// Optional: The path to the config file. Not in the toml, as wouldn't make sense.
	ConfigPath string `toml:"-" valid:"-"`

	// TODO: Enforce this when parsing this toml file
	Strict bool `toml:"STRICT" valid:"optional"`

	StellarCoreURL         string `toml:"-" valid:"-"`
	CaptiveCoreUseDB       bool   `toml:"-" valid:"-"`
	CaptiveCoreStoragePath string `toml:"CAPTIVE_CORE_STORAGE_PATH" valid:"optional"`
	StellarCoreBinaryPath  string `toml:"STELLAR_CORE_BINARY_PATH" valid:"optional"`
	CaptiveCoreConfigPath  string `toml:"CAPTIVE_CORE_CONFIG_PATH" valid:"optional"`
	CaptiveCoreHTTPPort    int    `toml:"CAPTIVE_CORE_HTTP_PORT" valid:"optional"`

	Endpoint                         string         `toml:"ENDPOINT" valid:"optional"`
	AdminEndpoint                    string         `toml:"ADMIN_ENDPOINT" valid:"optional"`
	CheckpointFrequency              uint32         `toml:"CHECKPOINT_FREQUENCY" valid:"optional"`
	CoreRequestTimeout               Duration       `toml:"CORE_REQUEST_TIMEOUT" valid:"optional"`
	DefaultEventsLimit               uint           `toml:"DEFAULT_EVENTS_LIMIT" valid:"optional"`
	EventLedgerRetentionWindow       PositiveUint32 `toml:"EVENT_LEDGER_RETENTION_WINDOW" valid:"optional"`
	FriendbotURL                     string         `toml:"FRIENDBOT_URL" valid:"optional"`
	HistoryArchiveURLs               []string       `toml:"HISTORY_ARCHIVE_URLS" valid:"required"`
	IngestionTimeout                 Duration       `toml:"INGESTION_TIMEOUT" valid:"optional"`
	LogFormat                        LogFormat      `toml:"LOG_FORMAT" valid:"optional"`
	LogLevel                         logrus.Level   `toml:"LOG_LEVEL" valid:"optional"`
	MaxEventsLimit                   uint           `toml:"MAX_EVENTS_LIMIT" valid:"optional"`
	MaxHealthyLedgerLatency          Duration       `toml:"MAX_HEALTHY_LEDGER_LATENCY" valid:"optional"`
	NetworkPassphrase                string         `toml:"NETWORK_PASSPHRASE" valid:"required"`
	PreflightWorkerCount             PositiveUint   `toml:"PREFLIGHT_WORKER_COUNT" valid:"optional"`
	PreflightWorkerQueueSize         PositiveUint   `toml:"PREFLIGHT_WORKER_QUEUE_SIZE" valid:"optional"`
	SQLiteDBPath                     string         `toml:"SQLITE_DB_PATH" valid:"optional"`
	TransactionLedgerRetentionWindow PositiveUint32 `toml:"TRANSACTION_LEDGER_RETENTION_WINDOW" valid:"optional"`
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
	// TODO: Test this works
	for _, option := range cfg.options() {
		reflect.ValueOf(option.ConfigKey).Elem().Set(reflect.ValueOf(option.DefaultValue))
	}
	return nil
}

func Read(path string, strict bool) (*Config, error) {
	cfg := &Config{}
	// TODO: Enforce strict parsing here
	err := support.Read(path, cfg)
	if err != nil {
		switch cause := errors.Cause(err).(type) {
		case *support.InvalidConfigError:
			return nil, errors.Wrap(cause, "config file")
		default:
			return nil, err
		}
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
