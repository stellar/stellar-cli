package loadtest

import (
	"github.com/spf13/cobra"
)

// Config represents the configuration of a load test to a soroban-rpc server
type Config struct {
	SorobanRPCURL          string
	TestDuration           string
	SpecGenerator          string
	RequestsPerSecond      int
	BatchInterval          string
	NetworkPassphrase      string
	GetEventsStartLedger   int32
	HelloWorldContractPath string
}

func (cfg *Config) AddFlags(cmd *cobra.Command) error {
	cmd.Flags().StringVarP(&cfg.SorobanRPCURL, "soroban-rpc-url", "u", "", "Endpoint to send JSON RPC requests to")
	if err := cmd.MarkFlagRequired("soroban-rpc-url"); err != nil {
		return err
	}

	cmd.Flags().StringVarP(&cfg.TestDuration, "duration", "d", "60s", "How long to generate load to the RPC server")
	cmd.Flags().StringVarP(&cfg.SpecGenerator, "spec-generator", "g", "getHealth", "Which spec generator to use to generate load")
	cmd.Flags().IntVarP(&cfg.RequestsPerSecond, "requests-per-second", "n", 10, "How many requests per second to send to the RPC server")
	cmd.Flags().StringVarP(&cfg.BatchInterval, "batch-interval", "i", "100ms", "How often to send a batch of requests")
	cmd.Flags().StringVarP(&cfg.NetworkPassphrase, "network-passphrase", "p", "Test SDF Network ; September 2015", "Network passphrase to use when simulating transactions")
	cmd.Flags().Int32Var(&cfg.GetEventsStartLedger, "get-events-start-ledger", 1, "Start ledger to fetch events after in GetEventsGenerator")
	cmd.Flags().StringVar(&cfg.HelloWorldContractPath, "hello-world-contract-path", "", "Location of hello world contract to use when simulating transactions")
	return nil
}
