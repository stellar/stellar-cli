package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
	supportconfig "github.com/stellar/go/support/config"
	"github.com/stellar/go/support/errors"
	goxdr "github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon"
)

func main() {
	var cfg, flags = config.Flags()

	rootCmd := &cobra.Command{
		Use:   "soroban-rpc",
		Short: "Start the remote soroban-rpc server",
		Run: func(_ *cobra.Command, _ []string) {
			if err := applyFlags(cfg, flags); err != nil {
				fmt.Fprintln(os.Stderr, err)
				os.Exit(1)
			}
			daemon.MustNew(cfg).Run()
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

	rootCmd.AddCommand(versionCmd)

	if err := flags.Init(rootCmd); err != nil {
		fmt.Fprintf(os.Stderr, "could not parse config options: %v\n", err)
		os.Exit(1)
	}

	if err := rootCmd.Execute(); err != nil {
		fmt.Fprintf(os.Stderr, "could not run: %v\n", err)

		os.Exit(1)
	}
}

func applyFlags(cfg *config.Config, flags supportconfig.ConfigOptions) error {
	err := flags.SetValues()
	if err != nil {
		return err
	}
	if cfg.ConfigPath != "" {
		fileConfig, err := config.Read(cfg.ConfigPath)
		if err != nil {
			return errors.Wrap(err, "reading config file")
		}
		*cfg, err = config.Merge(fileConfig, cfg)
		if err != nil {
			return errors.Wrap(err, "merging config file")
		}
	}

	fmt.Printf("Merged config: %+v\n", cfg)

	err = cfg.Validate()
	if err != nil {
		return err
	}
	return nil
}
