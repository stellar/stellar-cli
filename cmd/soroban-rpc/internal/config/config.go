package config

import (
	"bytes"
	"fmt"
	"go/types"
	"math"
	"os"
	"runtime"
	"strings"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"

	"github.com/stellar/go/network"
	supportconfig "github.com/stellar/go/support/config"
	"github.com/stellar/go/support/errors"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
)

type LogFormat int

const (
	LogFormatText = iota
	LogFormatJSON
)

type DaemonConfig struct {
	Endpoint                         string        `toml:"endpoint" valid:"optional"`
	AdminEndpoint                    string        `toml:"admin_endpoint" valid:"optional"`
	IngestionTimeoutMinutes          uint          `toml:"ingestion_timeout_minutes" valid:"optional"`
	CoreTimeoutSeconds               uint          `toml:"core_timeout_seconds" valid:"optional"`
	MaxHealthyLedgerLatencySeconds   uint          `toml:"max_healthy_ledger_latency_seconds" valid:"optional"`
	CheckpointFrequency              uint32        `toml:"checkpoint_frequency" valid:"optional"`
	CoreRequestTimeout               time.Duration `toml:"core_request_timeout" valid:"optional"`
	DefaultEventsLimit               uint          `toml:"default_events_limit" valid:"optional"`
	EventLedgerRetentionWindow       uint32        `toml:"event_ledger_retention_window" valid:"optional"`
	FriendbotURL                     string        `toml:"friendbot_url" valid:"optional"`
	HistoryArchiveURLs               []string      `toml:"history_archive_urls" valid:"required"`
	IngestionTimeout                 time.Duration `toml:"ingestion_timeout" valid:"optional"`
	LogFormat                        LogFormat     `toml:"log_format" valid:"optional"`
	LogLevel                         logrus.Level  `toml:"log_level" valid:"optional"`
	MaxEventsLimit                   uint          `toml:"max_events_limit" valid:"optional"`
	MaxHealthyLedgerLatency          time.Duration `toml:"max_healthy_ledger_latency" valid:"optional"`
	NetworkPassphrase                string        `toml:"network_passphrase" valid:"required"`
	PreflightWorkerCount             uint          `toml:"preflight_worker_count" valid:"optional"`
	PreflightWorkerQueueSize         uint          `toml:"preflight_worker_queue_size" valid:"optional"`
	SQLiteDBPath                     string        `toml:"sqlite_db_path" valid:"optional"`
	TransactionLedgerRetentionWindow uint32        `toml:"transaction_ledger_retention_window" valid:"optional"`
}

type CaptiveCoreConfig struct {
	CaptiveCoreConfigPath  string `toml:"captive_core_config_path" valid:"required"`
	CaptiveCoreHTTPPort    uint16 `toml:"captive_core_http_port" valid:"optional"`
	CaptiveCoreStoragePath string `toml:"captive_core_storage_path" valid:"optional"`
	CaptiveCoreUseDB       bool   `toml:"captive_core_use_db" valid:"optional"`
	StellarCoreBinaryPath  string `toml:"stellar_core_binary_path" valid:"required"`
	StellarCoreURL         string `toml:"stellar_core_url" valid:"optional"`
}

// Config represents the configuration of a friendbot server
type Config struct {
	// Optional: The path to the config file. Not in the toml, as wouldn't make sense.
	ConfigPath string

	DaemonConfig      `toml:"-"`
	CaptiveCoreConfig `toml:"stellar-core" valid:"optional"`
}

func (cfg *Config) Require() {
	cfg.options().Require()
}

func (cfg *Config) SetValues() error {
	return cfg.options().SetValues()
}

func (cfg *Config) Init(cmd *cobra.Command) error {
	err := cfg.options().Init(cmd)
	if err != nil {
		return err
	}
	cfg.setDefaults()
	return cfg.loadFile()
}

func (cfg *Config) setDefaults() {
	if cfg.StellarCoreURL == "" {
		cfg.StellarCoreURL = fmt.Sprintf("http://localhost:%d", cfg.CaptiveCoreHTTPPort)
	}
	cfg.IngestionTimeout = time.Duration(cfg.IngestionTimeoutMinutes) * time.Minute
	cfg.CoreRequestTimeout = time.Duration(cfg.CoreTimeoutSeconds) * time.Second
	cfg.MaxHealthyLedgerLatency = time.Duration(cfg.MaxHealthyLedgerLatencySeconds) * time.Second
}

func (cfg *Config) loadFile() error {
	if cfg.ConfigPath == "" {
		return nil
	}
	var fileConfig Config
	err := supportconfig.Read(cfg.ConfigPath, &fileConfig)
	if err != nil {
		switch cause := errors.Cause(err).(type) {
		case *supportconfig.InvalidConfigError:
			return errors.Wrap(cause, "config file")
		default:
			return err
		}
	}
	*cfg, err = cfg.Merge(&fileConfig)
	return err
}

func (cfg *Config) Validate() error {
	if cfg.DefaultEventsLimit > cfg.MaxEventsLimit {
		return fmt.Errorf(
			"default-events-limit (%v) cannot exceed max-events-limit (%v)\n",
			cfg.DefaultEventsLimit,
			cfg.MaxEventsLimit,
		)
	}

	return nil
}

// Merge other into cfg, overriding local values with other values. Neither
// config is modified, instead a new config is returned.
// TODO: Unit-test this
func (cfg *Config) Merge(other *Config) (Config, error) {
	var buf bytes.Buffer
	err := toml.NewEncoder(&buf).Encode(cfg)
	if err != nil {
		return Config{}, errors.Wrap(err, "encoding config")
	}
	err = toml.NewEncoder(&buf).Encode(other)
	if err != nil {
		return Config{}, errors.Wrap(err, "encoding config")
	}

	var merged Config
	_, err = toml.Decode(buf.String(), &cfg)
	if err != nil {
		return Config{}, errors.Wrap(err, "decoding config")
	}

	merged.ConfigPath = cfg.ConfigPath
	if other.ConfigPath != "" {
		merged.ConfigPath = other.ConfigPath
	}

	return merged, nil
}

func (cfg *Config) options() supportconfig.ConfigOptions {
	return supportconfig.ConfigOptions{
		{
			Name:        "config-path",
			EnvVar:      "SOROBAN_RPC_CONFIG_PATH",
			Usage:       "File path to the toml configuration file",
			OptType:     types.String,
			ConfigKey:   &cfg.ConfigPath,
			FlagDefault: "",
			Required:    false,
		},
		{
			Name:        "endpoint",
			Usage:       "Endpoint to listen and serve on",
			OptType:     types.String,
			ConfigKey:   &cfg.Endpoint,
			FlagDefault: "localhost:8000",
			Required:    false,
		},
		{
			Name:        "admin-endpoint",
			Usage:       "Admin endpoint to listen and serve on. WARNING: this should not be accessible from the Internet and does not use TLS. \"\" (default) disables the admin server",
			OptType:     types.String,
			ConfigKey:   &cfg.AdminEndpoint,
			FlagDefault: "",
			Required:    false,
		},
		{
			Name:        "stellar-core-url",
			ConfigKey:   &cfg.StellarCoreURL,
			OptType:     types.String,
			Required:    false,
			FlagDefault: "",
			Usage:       "URL used to query Stellar Core (local captive core by default)",
		},
		{
			Name:        "stellar-core-timeout-seconds",
			Usage:       "Timeout used when submitting requests to stellar-core",
			OptType:     types.Uint,
			ConfigKey:   &cfg.CoreTimeoutSeconds,
			FlagDefault: uint(2),
			Required:    false,
		},
		{
			Name:        "stellar-captive-core-http-port",
			ConfigKey:   &cfg.CaptiveCoreHTTPPort,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(11626),
			Usage:       "HTTP port for Captive Core to listen on (0 disables the HTTP server)",
		},
		{
			Name:        "log-level",
			ConfigKey:   &cfg.LogLevel,
			OptType:     types.String,
			FlagDefault: "info",
			CustomSetValue: func(co *supportconfig.ConfigOption) error {
				ll, err := logrus.ParseLevel(viper.GetString(co.Name))
				if err != nil {
					return fmt.Errorf("could not parse log-level: %v", viper.GetString(co.Name))
				}
				*(co.ConfigKey.(*logrus.Level)) = ll
				return nil
			},
			Usage: "minimum log severity (debug, info, warn, error) to log",
		},
		{
			Name:        "log-format",
			OptType:     types.String,
			FlagDefault: "text",
			Required:    false,
			Usage:       "format used for output logs (json or text)",
			ConfigKey:   &cfg.LogFormat,
			CustomSetValue: func(co *supportconfig.ConfigOption) error {
				logFormatStr := viper.GetString(co.Name)
				switch logFormatStr {
				case "text":
					*(co.ConfigKey.(*LogFormat)) = LogFormatText
				case "json":
					*(co.ConfigKey.(*LogFormat)) = LogFormatJSON
				default:
					return fmt.Errorf("invalid log-format: %v", logFormatStr)
				}
				return nil
			},
		},
		{
			Name:        "stellar-core-binary-path",
			OptType:     types.String,
			FlagDefault: "",
			Required:    true,
			Usage:       "path to stellar core binary",
			ConfigKey:   &cfg.StellarCoreBinaryPath,
		},
		{
			Name:        "captive-core-config-path",
			OptType:     types.String,
			FlagDefault: "",
			Required:    true,
			Usage:       "path to additional configuration for the Stellar Core configuration file used by captive core. It must, at least, include enough details to define a quorum set",
			ConfigKey:   &cfg.CaptiveCoreConfigPath,
		},
		{
			Name:    "captive-core-storage-path",
			OptType: types.String,
			CustomSetValue: func(opt *supportconfig.ConfigOption) error {
				existingValue := viper.GetString(opt.Name)
				if existingValue == "" || existingValue == "." {
					cwd, err := os.Getwd()
					if err != nil {
						return fmt.Errorf("Unable to determine the current directory: %s", err)
					}
					existingValue = cwd
				}
				*opt.ConfigKey.(*string) = existingValue
				return nil
			},
			Required:  false,
			Usage:     "Storage location for Captive Core bucket data",
			ConfigKey: &cfg.CaptiveCoreStoragePath,
		},
		{
			Name:        "captive-core-use-db",
			OptType:     types.Bool,
			FlagDefault: false,
			Required:    false,
			Usage:       "informs captive core to use on disk mode. the db will by default be created in current runtime directory of soroban-rpc, unless DATABASE=<path> setting is present in captive core config file.",
			ConfigKey:   &cfg.CaptiveCoreUseDB,
		},
		{
			Name:        "history-archive-urls",
			ConfigKey:   &cfg.HistoryArchiveURLs,
			OptType:     types.String,
			Required:    true,
			FlagDefault: "",
			CustomSetValue: func(co *supportconfig.ConfigOption) error {
				stringOfUrls := viper.GetString(co.Name)
				urlStrings := strings.Split(stringOfUrls, ",")

				*(co.ConfigKey.(*[]string)) = urlStrings
				return nil
			},
			Usage: "comma-separated list of stellar history archives to connect with",
		},
		{
			Name:      "friendbot-url",
			Usage:     "The friendbot URL to be returned by getNetwork endpoint",
			OptType:   types.String,
			ConfigKey: &cfg.FriendbotURL,
			Required:  false,
		},
		{
			Name:        "network-passphrase",
			Usage:       "Network passphrase of the Stellar network transactions should be signed for",
			OptType:     types.String,
			ConfigKey:   &cfg.NetworkPassphrase,
			FlagDefault: network.FutureNetworkPassphrase,
			Required:    true,
		},
		{
			Name:        "db-path",
			Usage:       "SQLite DB path",
			OptType:     types.String,
			ConfigKey:   &cfg.SQLiteDBPath,
			FlagDefault: "soroban_rpc.sqlite",
			Required:    false,
		},
		{
			Name:        "ingestion-timeout-minutes",
			Usage:       "Ingestion Timeout when bootstrapping data (checkpoint and in-memory initialization) and preparing ledger reads",
			OptType:     types.Uint,
			ConfigKey:   &cfg.IngestionTimeoutMinutes,
			FlagDefault: uint(30),
			Required:    false,
		},
		{
			Name:        "checkpoint-frequency",
			Usage:       "establishes how many ledgers exist between checkpoints, do NOT change this unless you really know what you are doing",
			OptType:     types.Uint32,
			ConfigKey:   &cfg.CheckpointFrequency,
			FlagDefault: uint32(64),
			Required:    false,
		},
		{
			Name:        "event-retention-window",
			OptType:     types.Uint32,
			FlagDefault: uint32(ledgerbucketwindow.DefaultEventLedgerRetentionWindow),
			Required:    false,
			Usage: fmt.Sprintf("configures the event retention window expressed in number of ledgers,"+
				" the default value is %d which corresponds to about 24 hours of history", ledgerbucketwindow.DefaultEventLedgerRetentionWindow),
			ConfigKey:      &cfg.EventLedgerRetentionWindow,
			CustomSetValue: mustPositiveUint32,
		},
		{
			Name:        "transaction-retention-window",
			OptType:     types.Uint32,
			FlagDefault: uint32(1440),
			Required:    false,
			Usage: "configures the transaction retention window expressed in number of ledgers," +
				" the default value is 1440 which corresponds to about 2 hours of history",
			ConfigKey:      &cfg.TransactionLedgerRetentionWindow,
			CustomSetValue: mustPositiveUint32,
		},
		{
			Name:        "max-events-limit",
			ConfigKey:   &cfg.MaxEventsLimit,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(10000),
			Usage:       "Maximum amount of events allowed in a single getEvents response",
		},
		{
			Name:        "default-events-limit",
			ConfigKey:   &cfg.DefaultEventsLimit,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(100),
			Usage:       "Default cap on the amount of events included in a single getEvents response",
		},
		{
			Name: "max-healthy-ledger-latency-seconds",
			Usage: "maximum ledger latency (i.e. time elapsed since the last known ledger closing time) considered to be healthy" +
				" (used for the /health endpoint)",
			OptType:     types.Uint,
			ConfigKey:   &cfg.MaxHealthyLedgerLatencySeconds,
			FlagDefault: uint(30),
			Required:    false,
		},
		{
			Name:           "preflight-worker-count",
			ConfigKey:      &cfg.PreflightWorkerCount,
			OptType:        types.Uint,
			Required:       false,
			FlagDefault:    uint(runtime.NumCPU()),
			Usage:          "Number of workers (read goroutines) used to compute preflights for the simulateTransaction endpoint. Defaults to the number of CPUs.",
			CustomSetValue: mustPositiveUint32,
		},
		{
			Name:        "preflight-worker-queue-size",
			ConfigKey:   &cfg.PreflightWorkerQueueSize,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(runtime.NumCPU()),
			Usage:       "Maximum number of outstanding preflight requests for the simulateTransaction endpoint. Defaults to the number of CPUs.",
		},
	}
}

func mustPositiveUint32(co *supportconfig.ConfigOption) error {
	v := viper.GetInt(co.Name)
	if v <= 0 {
		return fmt.Errorf("%s must be positive", co.Name)
	}
	if v > math.MaxUint32 {
		return fmt.Errorf("%s is too large (must be <= %d)", co.Name, math.MaxUint32)
	}
	*(co.ConfigKey.(*uint32)) = uint32(v)
	return nil
}
