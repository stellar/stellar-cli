package main

import (
	"fmt"
	"go/types"
	"os"

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
	var endpoint, horizonURL, stellarCoreURL, networkPassphrase string
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
		&config.ConfigOption{
			Name:        "stellar-core-url",
			ConfigKey:   &stellarCoreURL,
			OptType:     types.String,
			Required:    true,
			FlagDefault: "",
			Usage:       "URL used to query Stellar Core",
		},
		&config.ConfigOption{
			Name:        "log-level",
			ConfigKey:   &logLevel,
			OptType:     types.String,
			FlagDefault: "info",
			CustomSetValue: func(co *config.ConfigOption) error {
				ll, err := logrus.ParseLevel(viper.GetString(co.Name))
				if err != nil {
					return fmt.Errorf("Could not parse log-level: %v", viper.GetString(co.Name))
				}
				*(co.ConfigKey.(*logrus.Level)) = ll
				return nil
			},
			Usage: "minimum log severity (debug, info, warn, error) to log",
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
	}
	cmd := &cobra.Command{
		Use:   "soroban-rpc",
		Short: "Run the remote soroban-rpc server",
		Run: func(_ *cobra.Command, _ []string) {
			configOpts.Require()
			configOpts.SetValues()

			config := localConfig.LocalConfig{
				EndPoint:          endpoint,
				HorizonURL:        horizonURL,
				StellarCoreURL:    stellarCoreURL,
				NetworkPassphrase: networkPassphrase,
				LogLevel:          logLevel,
				TxConcurrency:     txConcurrency,
				TxQueueSize:       txQueueSize,
			}
			exitCode := daemon.Start(config)
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
