package main

import (
	"fmt"
	"go/types"
	"os"
	"runtime"
	"strings"

	"github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"github.com/stellar/go/network"
	supportconfig "github.com/stellar/go/support/config"
	"github.com/stellar/go/support/errors"
	localConfig "github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
)

// Config represents the configuration of a friendbot server
type Config struct {
	// The path to the config file
	ConfigPath string

	Endpoint                       string `toml:"endpoint" valid:"optional"`
	AdminEndpoint                  string `toml:"admin-endpoint" valid:"optional"`
	CaptiveCoreHTTPPort            uint   `toml:"captive-core-http-port" valid:"optional"`
	IngestionTimeoutMinutes        uint   `toml:"ingestion-timeout-minutes" valid:"optional"`
	CoreTimeoutSeconds             uint   `toml:"core-timeout-seconds" valid:"optional"`
	MaxHealthyLedgerLatencySeconds uint   `toml:"max-healthy-ledger-latency-seconds" valid:"optional"`
	localConfig.LocalConfig        `toml:"-"`
}

func (cfg *Config) Read(path string) error {
	err := supportconfig.Read(path, cfg)
	if err != nil {
		switch cause := errors.Cause(err).(type) {
		case *supportconfig.InvalidConfigError:
			return errors.Wrap(cause, "config file")
		default:
			return err
		}
	}
	return nil
}

func (cfg *Config) Require() {
	cfg.options().Require()
}

func (cfg *Config) SetValues() error {
	return cfg.options().SetValues()
}

func (cfg *Config) Init(cmd *cobra.Command) error {
	return cfg.options().Init(cmd)
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
			ConfigKey:   &cfg.LocalConfig.StellarCoreURL,
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
			ConfigKey:   &cfg.LocalConfig.LogLevel,
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
			ConfigKey:   &cfg.LocalConfig.LogFormat,
			CustomSetValue: func(co *supportconfig.ConfigOption) error {
				logFormatStr := viper.GetString(co.Name)
				switch logFormatStr {
				case "text":
					*(co.ConfigKey.(*supportconfig.LogFormat)) = supportconfig.LogFormatText
				case "json":
					*(co.ConfigKey.(*supportconfig.LogFormat)) = supportconfig.LogFormatJSON
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
			ConfigKey:   &cfg.LocalConfig.StellarCoreBinaryPath,
		},
		{
			Name:        "captive-core-config-path",
			OptType:     types.String,
			FlagDefault: "",
			Required:    true,
			Usage:       "path to additional configuration for the Stellar Core configuration file used by captive core. It must, at least, include enough details to define a quorum set",
			ConfigKey:   &cfg.LocalConfig.CaptiveCoreConfigPath,
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
			ConfigKey: &cfg.LocalConfig.CaptiveCoreStoragePath,
		},
		{
			Name:        "captive-core-use-db",
			OptType:     types.Bool,
			FlagDefault: false,
			Required:    false,
			Usage:       "informs captive core to use on disk mode. the db will by default be created in current runtime directory of soroban-rpc, unless DATABASE=<path> setting is present in captive core config file.",
			ConfigKey:   &cfg.LocalConfig.CaptiveCoreUseDB,
		},
		{
			Name:        "history-archive-urls",
			ConfigKey:   &cfg.LocalConfig.HistoryArchiveURLs,
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
			ConfigKey: &cfg.LocalConfig.FriendbotURL,
			Required:  false,
		},
		{
			Name:        "network-passphrase",
			Usage:       "Network passphrase of the Stellar network transactions should be signed for",
			OptType:     types.String,
			ConfigKey:   &cfg.LocalConfig.NetworkPassphrase,
			FlagDefault: network.FutureNetworkPassphrase,
			Required:    true,
		},
		{
			Name:        "db-path",
			Usage:       "SQLite DB path",
			OptType:     types.String,
			ConfigKey:   &cfg.LocalConfig.SQLiteDBPath,
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
			ConfigKey:   &cfg.LocalConfig.CheckpointFrequency,
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
			ConfigKey:      &cfg.LocalConfig.EventLedgerRetentionWindow,
			CustomSetValue: mustPositiveUint32,
		},
		{
			Name:        "transaction-retention-window",
			OptType:     types.Uint32,
			FlagDefault: uint32(1440),
			Required:    false,
			Usage: "configures the transaction retention window expressed in number of ledgers," +
				" the default value is 1440 which corresponds to about 2 hours of history",
			ConfigKey:      &cfg.LocalConfig.TransactionLedgerRetentionWindow,
			CustomSetValue: mustPositiveUint32,
		},
		{
			Name:        "max-events-limit",
			ConfigKey:   &cfg.LocalConfig.MaxEventsLimit,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(10000),
			Usage:       "Maximum amount of events allowed in a single getEvents response",
		},
		{
			Name:        "default-events-limit",
			ConfigKey:   &cfg.LocalConfig.DefaultEventsLimit,
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
			Name:        "preflight-worker-count",
			ConfigKey:   &cfg.LocalConfig.PreflightWorkerCount,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(runtime.NumCPU()),
			Usage:       "Number of workers (read goroutines) used to compute preflights for the simulateTransaction endpoint. Defaults to the number of CPUs.",
		},
		{
			Name:        "preflight-worker-queue-size",
			ConfigKey:   &cfg.LocalConfig.PreflightWorkerQueueSize,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(runtime.NumCPU()),
			Usage:       "Maximum number of outstanding preflight requests for the simulateTransaction endpoint. Defaults to the number of CPUs.",
		},
	}
}
