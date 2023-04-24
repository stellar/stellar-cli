package config

import (
	"fmt"
	"go/types"
	"math"
	"os"
	"strings"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/spf13/viper"

	support "github.com/stellar/go/support/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
)

func Flags() (*Config, support.ConfigOptions) {
	cfg := &Config{}
	cfg.SetDefaults()
	return cfg, support.ConfigOptions{
		{
			Name:        "config-path",
			EnvVar:      "SOROBAN_RPC_CONFIG_PATH",
			Usage:       "File path to the toml configuration file",
			OptType:     types.String,
			ConfigKey:   &cfg.ConfigPath,
			FlagDefault: cfg.ConfigPath,
			Required:    false,
		},
		{
			Name:        "endpoint",
			Usage:       "Endpoint to listen and serve on",
			OptType:     types.String,
			ConfigKey:   &cfg.Endpoint,
			FlagDefault: cfg.Endpoint,
			Required:    false,
		},
		{
			Name:        "admin-endpoint",
			Usage:       "Admin endpoint to listen and serve on. WARNING: this should not be accessible from the Internet and does not use TLS. \"\" (default) disables the admin server",
			OptType:     types.String,
			ConfigKey:   &cfg.AdminEndpoint,
			FlagDefault: cfg.AdminEndpoint,
			Required:    false,
		},
		{
			Name:        "stellar-core-url",
			Usage:       "URL used to query Stellar Core (local captive core by default)",
			OptType:     types.String,
			ConfigKey:   &cfg.StellarCoreURL,
			FlagDefault: cfg.StellarCoreURL,
			Required:    false,
		},
		{
			Name:           "stellar-core-timeout-seconds",
			Usage:          "Timeout used when submitting requests to stellar-core",
			OptType:        types.String,
			ConfigKey:      &cfg.CoreRequestTimeout,
			FlagDefault:    cfg.CoreRequestTimeout.String(),
			Required:       false,
			CustomSetValue: mustDuration,
		},
		{
			Name:        "stellar-captive-core-http-port",
			Usage:       "HTTP port for Captive Core to listen on (0 disables the HTTP server)",
			OptType:     types.Uint,
			ConfigKey:   &cfg.CaptiveCoreConfig.HTTPPort,
			FlagDefault: cfg.CaptiveCoreConfig.HTTPPort,
			Required:    false,
		},
		{
			Name:        "log-level",
			Usage:       "minimum log severity (debug, info, warn, error) to log",
			OptType:     types.String,
			ConfigKey:   &cfg.LogLevel,
			FlagDefault: cfg.LogLevel.String(),
			Required:    false,
			CustomSetValue: func(co *support.ConfigOption) error {
				ll, err := logrus.ParseLevel(viper.GetString(co.Name))
				if err != nil {
					return fmt.Errorf("could not parse log-level: %v", viper.GetString(co.Name))
				}
				*(co.ConfigKey.(*logrus.Level)) = ll
				return nil
			},
		},
		{
			Name:        "log-format",
			Usage:       "format used for output logs (json or text)",
			OptType:     types.String,
			ConfigKey:   &cfg.LogFormat,
			FlagDefault: cfg.LogFormat.String(),
			Required:    false,
			CustomSetValue: func(co *support.ConfigOption) error {
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
			Usage:       "path to stellar core binary",
			OptType:     types.String,
			ConfigKey:   &cfg.StellarCoreBinaryPath,
			FlagDefault: cfg.StellarCoreBinaryPath,
			Required:    true,
		},
		{
			Name:        "captive-core-storage-path",
			Usage:       "Storage location for Captive Core bucket data",
			OptType:     types.String,
			ConfigKey:   &cfg.CaptiveCoreStoragePath,
			FlagDefault: cfg.CaptiveCoreStoragePath,
			Required:    false,
			CustomSetValue: func(opt *support.ConfigOption) error {
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
		},
		{
			Name:        "captive-core-use-db",
			Usage:       "informs captive core to use on disk mode. the db will by default be created in current runtime directory of soroban-rpc, unless DATABASE=<path> setting is present in captive core config file.",
			OptType:     types.Bool,
			ConfigKey:   &cfg.CaptiveCoreUseDB,
			FlagDefault: cfg.CaptiveCoreUseDB,
			Required:    false,
		},
		{
			Name:        "history-archive-urls",
			Usage:       "comma-separated list of stellar history archives to connect with",
			OptType:     types.String,
			ConfigKey:   &cfg.HistoryArchiveURLs,
			FlagDefault: strings.Join(cfg.HistoryArchiveURLs, ","),
			Required:    true,
			CustomSetValue: func(co *support.ConfigOption) error {
				stringOfUrls := viper.GetString(co.Name)
				if stringOfUrls == "" {
					return nil
				}
				urlStrings := strings.Split(stringOfUrls, ",")

				*(co.ConfigKey.(*[]string)) = urlStrings
				return nil
			},
		},
		{
			Name:        "friendbot-url",
			Usage:       "The friendbot URL to be returned by getNetwork endpoint",
			OptType:     types.String,
			ConfigKey:   &cfg.FriendbotURL,
			FlagDefault: cfg.FriendbotURL,
			Required:    false,
		},
		{
			Name:        "network-passphrase",
			Usage:       "Network passphrase of the Stellar network transactions should be signed for",
			OptType:     types.String,
			ConfigKey:   &cfg.NetworkPassphrase,
			FlagDefault: cfg.NetworkPassphrase,
			Required:    true,
		},
		{
			Name:        "db-path",
			Usage:       "SQLite DB path",
			OptType:     types.String,
			ConfigKey:   &cfg.SQLiteDBPath,
			FlagDefault: cfg.SQLiteDBPath,
			Required:    false,
		},
		{
			Name:           "ingestion-timeout",
			Usage:          "Ingestion Timeout when bootstrapping data (checkpoint and in-memory initialization) and preparing ledger reads",
			OptType:        types.String,
			ConfigKey:      &cfg.IngestionTimeout,
			FlagDefault:    cfg.IngestionTimeout.String(),
			Required:       false,
			CustomSetValue: mustDuration,
		},
		{
			Name:        "checkpoint-frequency",
			Usage:       "establishes how many ledgers exist between checkpoints, do NOT change this unless you really know what you are doing",
			OptType:     types.Uint32,
			ConfigKey:   &cfg.CheckpointFrequency,
			FlagDefault: cfg.CheckpointFrequency,
			Required:    false,
		},
		{
			Name: "event-retention-window",
			Usage: fmt.Sprintf("configures the event retention window expressed in number of ledgers,"+
				" the default value is %d which corresponds to about 24 hours of history", ledgerbucketwindow.DefaultEventLedgerRetentionWindow),
			OptType:        types.Uint32,
			ConfigKey:      &cfg.EventLedgerRetentionWindow,
			FlagDefault:    cfg.EventLedgerRetentionWindow,
			Required:       false,
			CustomSetValue: mustPositiveUint32,
		},
		{
			Name: "transaction-retention-window",
			Usage: "configures the transaction retention window expressed in number of ledgers," +
				" the default value is 1440 which corresponds to about 2 hours of history",
			OptType:        types.Uint32,
			ConfigKey:      &cfg.TransactionLedgerRetentionWindow,
			FlagDefault:    cfg.TransactionLedgerRetentionWindow,
			Required:       false,
			CustomSetValue: mustPositiveUint32,
		},
		{
			Name:        "max-events-limit",
			Usage:       "Maximum amount of events allowed in a single getEvents response",
			OptType:     types.Uint,
			ConfigKey:   &cfg.MaxEventsLimit,
			FlagDefault: cfg.MaxEventsLimit,
			Required:    false,
		},
		{
			Name:        "default-events-limit",
			Usage:       "Default cap on the amount of events included in a single getEvents response",
			OptType:     types.Uint,
			ConfigKey:   &cfg.DefaultEventsLimit,
			FlagDefault: cfg.DefaultEventsLimit,
			Required:    false,
		},
		{
			Name: "max-healthy-ledger-latency",
			Usage: "maximum ledger latency (i.e. time elapsed since the last known ledger closing time) considered to be healthy" +
				" (used for the /health endpoint)",
			OptType:        types.String,
			ConfigKey:      &cfg.MaxHealthyLedgerLatency,
			FlagDefault:    cfg.MaxHealthyLedgerLatency.String(),
			Required:       false,
			CustomSetValue: mustDuration,
		},
		{
			Name:           "preflight-worker-count",
			Usage:          "Number of workers (read goroutines) used to compute preflights for the simulateTransaction endpoint. Defaults to the number of CPUs.",
			OptType:        types.Uint,
			ConfigKey:      &cfg.PreflightWorkerCount,
			FlagDefault:    cfg.PreflightWorkerCount,
			Required:       false,
			CustomSetValue: mustPositiveUint,
		},
		{
			Name:        "preflight-worker-queue-size",
			Usage:       "Maximum number of outstanding preflight requests for the simulateTransaction endpoint. Defaults to the number of CPUs.",
			OptType:     types.Uint,
			ConfigKey:   &cfg.PreflightWorkerQueueSize,
			FlagDefault: cfg.PreflightWorkerQueueSize,
			Required:    false,
		},
	}
}

func mustPositiveUint(co *support.ConfigOption) error {
	v := viper.GetInt(co.Name)
	if v <= 0 {
		return fmt.Errorf("%s must be positive", co.Name)
	}
	*(co.ConfigKey.(*uint)) = uint(v)
	return nil
}

func mustPositiveUint32(co *support.ConfigOption) error {
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

func mustDuration(co *support.ConfigOption) error {
	v := viper.GetString(co.Name)
	d, err := time.ParseDuration(v)
	if err != nil {
		return err
	}
	*(co.ConfigKey.(*time.Duration)) = d
	return nil
}
