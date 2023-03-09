package main

import (
	"fmt"
	"go/types"
	"math"
	"os"
	"runtime"
	"strings"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"github.com/stellar/go/network"
	"github.com/stellar/go/support/config"
	goxdr "github.com/stellar/go/xdr"

	localConfig "github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon"
)

func mustPositiveUint32(co *config.ConfigOption) error {
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

func main() {
	var endpoint string
	var captiveCoreHTTPPort, ledgerEntryStorageTimeoutMinutes, coreTimeoutSeconds, maxHealthyLedgerLatencySeconds uint
	var serviceConfig localConfig.LocalConfig

	configOpts := config.ConfigOptions{
		{
			Name:        "endpoint",
			Usage:       "Endpoint to listen and serve on",
			OptType:     types.String,
			ConfigKey:   &endpoint,
			FlagDefault: "localhost:8000",
			Required:    false,
		},
		{
			Name:        "stellar-core-url",
			ConfigKey:   &serviceConfig.StellarCoreURL,
			OptType:     types.String,
			Required:    false,
			FlagDefault: "",
			Usage:       "URL used to query Stellar Core (local captive core by default)",
		},
		{
			Name:        "stellar-core-timeout-seconds",
			Usage:       "Timeout used when submitting requests to stellar-core",
			OptType:     types.Uint,
			ConfigKey:   &coreTimeoutSeconds,
			FlagDefault: uint(2),
			Required:    false,
		},
		{
			Name:        "stellar-captive-core-http-port",
			ConfigKey:   &captiveCoreHTTPPort,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(11626),
			Usage:       "HTTP port for Captive Core to listen on (0 disables the HTTP server)",
		},
		{
			Name:        "log-level",
			ConfigKey:   &serviceConfig.LogLevel,
			OptType:     types.String,
			FlagDefault: "info",
			CustomSetValue: func(co *config.ConfigOption) error {
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
			Name:        "stellar-core-binary-path",
			OptType:     types.String,
			FlagDefault: "",
			Required:    true,
			Usage:       "path to stellar core binary",
			ConfigKey:   &serviceConfig.StellarCoreBinaryPath,
		},
		{
			Name:        "captive-core-config-path",
			OptType:     types.String,
			FlagDefault: "",
			Required:    true,
			Usage:       "path to additional configuration for the Stellar Core configuration file used by captive core. It must, at least, include enough details to define a quorum set",
			ConfigKey:   &serviceConfig.CaptiveCoreConfigPath,
		},
		{
			Name:    "captive-core-storage-path",
			OptType: types.String,
			CustomSetValue: func(opt *config.ConfigOption) error {
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
			ConfigKey: &serviceConfig.CaptiveCoreStoragePath,
		},
		{
			Name:        "captive-core-use-db",
			OptType:     types.Bool,
			FlagDefault: false,
			Required:    false,
			Usage:       "informs captive core to use on disk mode. the db will by default be created in current runtime directory of soroban-rpc, unless DATABASE=<path> setting is present in captive core config file.",
			ConfigKey:   &serviceConfig.CaptiveCoreUseDB,
		},
		&config.ConfigOption{
			Name:        "history-archive-urls",
			ConfigKey:   &serviceConfig.HistoryArchiveURLs,
			OptType:     types.String,
			Required:    true,
			FlagDefault: "",
			CustomSetValue: func(co *config.ConfigOption) error {
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
			ConfigKey: &serviceConfig.FriendbotURL,
			Required:  false,
		},
		{
			Name:        "network-passphrase",
			Usage:       "Network passphrase of the Stellar network transactions should be signed for",
			OptType:     types.String,
			ConfigKey:   &serviceConfig.NetworkPassphrase,
			FlagDefault: network.FutureNetworkPassphrase,
			Required:    true,
		},
		{
			Name:        "db-path",
			Usage:       "SQLite DB path",
			OptType:     types.String,
			ConfigKey:   &serviceConfig.SQLiteDBPath,
			FlagDefault: "soroban_rpc.sqlite",
			Required:    false,
		},
		{
			Name:        "ledgerstorage-timeout-minutes",
			Usage:       "Ledger Entry Storage Timeout (when bootstrapping and reading each ledger)",
			OptType:     types.Uint,
			ConfigKey:   &ledgerEntryStorageTimeoutMinutes,
			FlagDefault: uint(30),
			Required:    false,
		},
		{
			Name:        "checkpoint-frequency",
			Usage:       "establishes how many ledgers exist between checkpoints, do NOT change this unless you really know what you are doing",
			OptType:     types.Uint32,
			ConfigKey:   &serviceConfig.CheckpointFrequency,
			FlagDefault: uint32(64),
			Required:    false,
		},
		{
			Name:        "event-retention-window",
			OptType:     types.Uint32,
			FlagDefault: uint32(17280),
			Required:    false,
			Usage: "configures the event retention window expressed in number of ledgers," +
				" the default value is 17280 which corresponds to about 24 hours of history",
			ConfigKey:      &serviceConfig.EventLedgerRetentionWindow,
			CustomSetValue: mustPositiveUint32,
		},
		{
			Name:        "transaction-retention-window",
			OptType:     types.Uint32,
			FlagDefault: uint32(1440),
			Required:    false,
			Usage: "configures the transaction retention window expressed in number of ledgers," +
				" the default value is 1440 which corresponds to about 2 hours of history",
			ConfigKey:      &serviceConfig.TransactionLedgerRetentionWindow,
			CustomSetValue: mustPositiveUint32,
		},
		{
			Name:        "max-events-limit",
			ConfigKey:   &serviceConfig.MaxEventsLimit,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(10000),
			Usage:       "Maximum amount of events allowed in a single getEvents response",
		},
		{
			Name:        "default-events-limit",
			ConfigKey:   &serviceConfig.DefaultEventsLimit,
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
			ConfigKey:   &maxHealthyLedgerLatencySeconds,
			FlagDefault: uint(30),
			Required:    false,
		},
		{
			Name:        "preflight-worker-count",
			ConfigKey:   &serviceConfig.PreflightWorkerCount,
			OptType:     types.Uint,
			Required:    false,
			FlagDefault: uint(runtime.NumCPU()),
			Usage:       "Number of workers (read goroutines) used to compute preflights",
		},
	}
	cmd := &cobra.Command{
		Use:   "soroban-rpc",
		Short: "Start the remote soroban-rpc server",
		Run: func(_ *cobra.Command, _ []string) {
			configOpts.Require()
			err := configOpts.SetValues()
			if err != nil {
				fmt.Fprintf(os.Stderr, "failed to set values : %v\n", err)
				os.Exit(1)
			}
			if serviceConfig.DefaultEventsLimit > serviceConfig.MaxEventsLimit {
				fmt.Fprintf(os.Stderr,
					"default-events-limit (%v) cannot exceed max-events-limit (%v)\n",
					serviceConfig.DefaultEventsLimit,
					serviceConfig.MaxEventsLimit,
				)
				os.Exit(1)
			}
			if serviceConfig.PreflightWorkerCount < 1 {
				fmt.Fprintln(os.Stderr, "preflight-worker-count must be > 0")
				os.Exit(1)
			}

			serviceConfig.CaptiveCoreHTTPPort = uint16(captiveCoreHTTPPort)
			if serviceConfig.StellarCoreURL == "" {
				serviceConfig.StellarCoreURL = fmt.Sprintf("http://localhost:%d", captiveCoreHTTPPort)
			}
			serviceConfig.LedgerEntryStorageTimeout = time.Duration(ledgerEntryStorageTimeoutMinutes) * time.Minute
			serviceConfig.CoreRequestTimeout = time.Duration(coreTimeoutSeconds) * time.Second
			serviceConfig.MaxHealthyLedgerLatency = time.Duration(maxHealthyLedgerLatencySeconds) * time.Second
			daemon.Run(serviceConfig, endpoint)
		},
	}

	versionCmd := &cobra.Command{
		Use:   "version",
		Short: "Print version information and exit",
		Run: func(_ *cobra.Command, _ []string) {
			if localConfig.CommitHash == "" {
				fmt.Printf("soroban-rpc dev\n")
			} else {
				// avoid printing the branch for the main branch
				// ( since that's what the end-user would typically have )
				// but keep it for internal build ( so that we'll know from which branch it
				// was built )
				branch := localConfig.Branch
				if branch == "main" {
					branch = ""
				}
				fmt.Printf("soroban-rpc %s (%s) %s\n", localConfig.Version, localConfig.CommitHash, branch)
				fmt.Printf("stellar-xdr %s\n", goxdr.CommitHash)
			}
		},
	}

	cmd.AddCommand(versionCmd)

	if err := configOpts.Init(cmd); err != nil {
		fmt.Fprintf(os.Stderr, "could not parse config options: %v\n", err)
		os.Exit(1)
	}

	if err := cmd.Execute(); err != nil {
		fmt.Fprintf(os.Stderr, "could not run: %v\n", err)

		os.Exit(1)
	}
}
