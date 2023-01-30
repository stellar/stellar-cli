package main

import (
	"fmt"
	"go/types"
	"os"
	"strings"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"github.com/stellar/go/network"
	"github.com/stellar/go/support/config"
	supportlog "github.com/stellar/go/support/log"
	goxdr "github.com/stellar/go/xdr"

	localConfig "github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon"
)

func main() {
	var endpoint, horizonURL, stellarCoreURL, binaryPath, configPath, friendbotURL, networkPassphrase, dbPath, captivecoreStoragePath string
	var captiveCoreHTTPPort, ledgerEntryStorageTimeoutMinutes uint
	var checkpointFrequency uint32
	var useDB bool
	var historyArchiveURLs []string
	var txConcurrency, txQueueSize int
	var logLevel logrus.Level

	configOpts := config.ConfigOptions{
		{
			Name:        "endpoint",
			Usage:       "Endpoint to listen and serve on",
			OptType:     types.String,
			ConfigKey:   &endpoint,
			FlagDefault: "localhost:8000",
			Required:    false,
		},
		&config.ConfigOption{
			Name:        "horizon-url",
			ConfigKey:   &horizonURL,
			OptType:     types.String,
			Required:    true,
			FlagDefault: "",
			Usage:       "URL used to query Horizon",
		},
		{
			Name:        "stellar-core-url",
			ConfigKey:   &stellarCoreURL,
			OptType:     types.String,
			Required:    true,
			FlagDefault: "http://localhost:11626",
			Usage:       "URL used to query Stellar Core (local captive core by default)",
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
			ConfigKey:   &logLevel,
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
			ConfigKey:   &binaryPath,
		},
		{
			Name:        "captive-core-config-path",
			OptType:     types.String,
			FlagDefault: "",
			Required:    true,
			Usage:       "path to additional configuration for the Stellar Core configuration file used by captive core. It must, at least, include enough details to define a quorum set",
			ConfigKey:   &configPath,
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
			ConfigKey: &captivecoreStoragePath,
		},
		{
			Name:        "captive-core-use-db",
			OptType:     types.Bool,
			FlagDefault: false,
			Required:    false,
			Usage:       "informs captive core to use on disk mode. the db will by default be created in current runtime directory of soroban-rpc, unless DATABASE=<path> setting is present in captive core config file.",
			ConfigKey:   &useDB,
		},
		&config.ConfigOption{
			Name:        "history-archive-urls",
			ConfigKey:   &historyArchiveURLs,
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
			Usage:     "URL where friendbot requests should be sent",
			OptType:   types.String,
			ConfigKey: &friendbotURL,
			Required:  false,
		},
		{
			Name:        "network-passphrase",
			Usage:       "Network passphrase of the Stellar network transactions should be signed for",
			OptType:     types.String,
			ConfigKey:   &networkPassphrase,
			FlagDefault: network.FutureNetworkPassphrase,
			Required:    true,
		},
		{
			Name:        "tx-concurrency",
			Usage:       "Maximum number of concurrent transaction submissions",
			OptType:     types.Int,
			ConfigKey:   &txConcurrency,
			FlagDefault: 10,
			Required:    false,
		},
		{
			Name:        "tx-queue",
			Usage:       "Maximum length of pending transactions queue",
			OptType:     types.Int,
			ConfigKey:   &txQueueSize,
			FlagDefault: 10,
			Required:    false,
		},
		{
			Name:        "db-path",
			Usage:       "SQLite DB path",
			OptType:     types.String,
			ConfigKey:   &dbPath,
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
			ConfigKey:   &checkpointFrequency,
			FlagDefault: uint32(64),
			Required:    false,
		},
	}
	cmd := &cobra.Command{
		Use:   "soroban-rpc",
		Short: "Start the remote soroban-rpc server",
		Run: func(_ *cobra.Command, _ []string) {
			configOpts.Require()
			err := configOpts.SetValues()
			if err != nil {
				fmt.Printf("failed to set values : %v\n", err)
				os.Exit(-1)
			}
			config := localConfig.LocalConfig{
				HorizonURL:                horizonURL,
				StellarCoreURL:            stellarCoreURL,
				StellarCoreBinaryPath:     binaryPath,
				CaptiveCoreConfigPath:     configPath,
				CaptiveCoreUseDB:          useDB,
				CaptiveCoreHTTPPort:       uint16(captiveCoreHTTPPort),
				CaptiveCoreStoragePath:    captivecoreStoragePath,
				FriendbotURL:              friendbotURL,
				NetworkPassphrase:         networkPassphrase,
				HistoryArchiveURLs:        historyArchiveURLs,
				LogLevel:                  logLevel,
				TxConcurrency:             txConcurrency,
				TxQueueSize:               txQueueSize,
				LedgerEntryStorageTimeout: time.Duration(ledgerEntryStorageTimeoutMinutes) * time.Minute,
				SQLiteDBPath:              dbPath,
				CheckpointFrequency:       checkpointFrequency,
			}
			exitCode := daemon.Run(config, endpoint)
			os.Exit(exitCode)
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
		supportlog.New().WithError(err).Fatal("could not parse config options")
		os.Exit(-1)
	}

	if err := cmd.Execute(); err != nil {
		supportlog.New().WithError(err).Fatal("could not run")
		os.Exit(-1)
	}
}
