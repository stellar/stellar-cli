package main

import (
	"fmt"
	"math"
	"os"
	"time"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	supportconfig "github.com/stellar/go/support/config"
	goxdr "github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon"
)

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

func main() {
	var configOpts Config
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
			if configOpts.DefaultEventsLimit > configOpts.MaxEventsLimit {
				fmt.Fprintf(os.Stderr,
					"default-events-limit (%v) cannot exceed max-events-limit (%v)\n",
					configOpts.DefaultEventsLimit,
					configOpts.MaxEventsLimit,
				)
				os.Exit(1)
			}
			if configOpts.PreflightWorkerCount < 1 {
				fmt.Fprintln(os.Stderr, "preflight-worker-count must be > 0")
				os.Exit(1)
			}

			configOpts.LocalConfig.CaptiveCoreHTTPPort = uint16(configOpts.CaptiveCoreHTTPPort)
			if configOpts.StellarCoreURL == "" {
				configOpts.StellarCoreURL = fmt.Sprintf("http://localhost:%d", configOpts.CaptiveCoreHTTPPort)
			}
			configOpts.IngestionTimeout = time.Duration(configOpts.IngestionTimeoutMinutes) * time.Minute
			configOpts.CoreRequestTimeout = time.Duration(configOpts.CoreTimeoutSeconds) * time.Second
			configOpts.MaxHealthyLedgerLatency = time.Duration(configOpts.MaxHealthyLedgerLatencySeconds) * time.Second
			daemon.MustNew(configOpts.LocalConfig, configOpts.Endpoint, configOpts.AdminEndpoint).Run()
		},
	}

	versionCmd := &cobra.Command{
		Use:   "version",
		Short: "Print version information and exit",
		Run: func(_ *cobra.Command, _ []string) {
			if config.CommitHash == "" {
				fmt.Printf("soroban-rpc dev\n")
			} else {
				// avoid printing the branch for the main branch
				// ( since that's what the end-user would typically have )
				// but keep it for internal build ( so that we'll know from which branch it
				// was built )
				branch := config.Branch
				if branch == "main" {
					branch = ""
				}
				fmt.Printf("soroban-rpc %s (%s) %s\n", config.Version, config.CommitHash, branch)
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
