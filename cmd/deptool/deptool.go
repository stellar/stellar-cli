package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

var versionCheck bool
var projectDir string
var writeChanges bool
var writeChangesInPlace bool

var rootCmd = &cobra.Command{
	Use:   "deptool",
	Short: "Repository dependency tool",
	Long:  `Repository dependency tool`,
	Run: func(cmd *cobra.Command, args []string) {
		if versionCheck {
			fmt.Println("Build version: 1.0")
			return
		}

		//If no arguments passed, we should fallback to help
		cmd.HelpFunc()(cmd, args)
	},
}

var scanCmd = &cobra.Command{
	Use:   "scan",
	Short: "scan project dependencies",
	Run: func(cmd *cobra.Command, args []string) {
		deps := scanProject(projectDir)
		printDependencies(deps)
	},
}

var analyzeCmd = &cobra.Command{
	Use:   "analyze",
	Short: "analyze project dependencies",
	Run: func(cmd *cobra.Command, args []string) {
		deps := scanProject(projectDir)
		analyzed := analyze(deps, analyzedDepPrinter)
		hasChanges := false
		// see if any of the dependencies could be upgraded.
		for _, dep := range analyzed {
			if dep.latestBranchCommit != dep.fullCommitHash {
				// yes, it could be upgraded.
				hasChanges = true
				break
			}
		}

		if hasChanges {
			if writeChanges || writeChangesInPlace {
				writeUpdates(projectDir, analyzed, writeChangesInPlace)
			}
			os.Exit(1)
		}
	},
}

func initCommandHandlers() {
	rootCmd.Flags().BoolVarP(&versionCheck, "version", "v", false, "Display and write current build version and exit")
	scanCmd.Flags().StringVarP(&projectDir, "directory", "d", ".", "The directory where the project resides")
	analyzeCmd.Flags().StringVarP(&projectDir, "directory", "d", ".", "The directory where the project resides")
	analyzeCmd.Flags().BoolVarP(&writeChanges, "write", "w", false, "Once analysis is complete, write out the proposed change to Cargo.toml.proposed and go.mod.proposed")
	analyzeCmd.Flags().BoolVarP(&writeChangesInPlace, "writeInPlace", "p", false, "Once analysis is complete, write out the changes to the existing Cargo.toml and go.mod")

	rootCmd.AddCommand(scanCmd)
	rootCmd.AddCommand(analyzeCmd)
}

func main() {
	initCommandHandlers()
	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		exitErr()
	}
}

func exitErr() {
	os.Exit(-1)
}
