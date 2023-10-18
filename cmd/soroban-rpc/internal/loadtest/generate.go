package loadtest

import (
	"context"
	"fmt"
	"sync"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/pkg/errors"
)

// Generates load to a soroban-rpc server based on configuration.
func GenerateLoad(cfg *Config) error {
	ch := jhttp.NewChannel(cfg.SorobanRPCURL, nil)
	client := jrpc2.NewClient(ch, nil)

	batchIntervalDur, err := time.ParseDuration(cfg.BatchInterval)
	if err != nil {
		return errors.Wrapf(err, "invalid time format for batch interval: %s", cfg.BatchInterval)
	}
	loadTestDuration, err := time.ParseDuration(cfg.TestDuration)
	if err != nil {
		return errors.Wrapf(err, "invalid time format for test duration: %s", cfg.TestDuration)
	}
	numBatches := int(loadTestDuration.Seconds() / batchIntervalDur.Seconds())

	// Generate request batches
	nameToRegisteredSpecGenerator := make(map[string]SpecGenerator)
	nameToRegisteredSpecGenerator["getHealth"] = &GetHealthGenerator{}
	nameToRegisteredSpecGenerator["getEvents"] = &GetEventsGenerator{
		startLedger: cfg.GetEventsStartLedger,
	}
	nameToRegisteredSpecGenerator["simulateTransaction"] = &SimulateTransactionGenerator{
		networkPassphrase:      cfg.NetworkPassphrase,
		helloWorldContractPath: cfg.HelloWorldContractPath,
	}
	generator, ok := nameToRegisteredSpecGenerator[cfg.SpecGenerator]
	if !ok {
		return errors.Wrapf(err, "spec generator with name %s does not exist", cfg.SpecGenerator)
	}
	var requestBatches [][]jrpc2.Spec
	batchSize := int(float64(cfg.RequestsPerSecond) * batchIntervalDur.Seconds())
	for i := 0; i < numBatches; i++ {
		var currentBatch []jrpc2.Spec
		for i := 0; i < batchSize; i++ {
			spec, err := generator.GenerateSpec()
			if err != nil {
				return errors.Wrapf(err, "could not generate spec: %v\n", err)
			}
			currentBatch = append(currentBatch, spec)
		}
		requestBatches = append(requestBatches, currentBatch)
	}

	// Actually generate load.
	fmt.Printf("Generating approximately %d requests per second for %v\n", cfg.RequestsPerSecond, loadTestDuration)
	fmt.Printf(
		"Sending %d batches of %d requests each, every %v for %v\n",
		numBatches,
		batchSize,
		batchIntervalDur,
		loadTestDuration,
	)
	startTime := time.Now()
	numRequestsSent := 0
	now := time.Time{}
	lastBatchSentTime := time.Time{}
	currentBatchI := 0
	var batchMu sync.Mutex
	for now.Before(startTime.Add(loadTestDuration)) && currentBatchI < len(requestBatches) {
		now = time.Now()
		if now.After(lastBatchSentTime.Add(batchIntervalDur)) {
			go func() {
				// Ignore response content for now.
				batchMu.Lock()
				if currentBatchI >= len(requestBatches) {
					batchMu.Unlock()
					return
				}
				currentBatch := requestBatches[currentBatchI]
				batchMu.Unlock()
				_, err := client.Batch(context.Background(), currentBatch)
				if err != nil {
					fmt.Printf("Batch call failed: %v\n", err)
					return
				}
			}()
			lastBatchSentTime = now
			numRequestsSent += batchSize

			batchMu.Lock()
			currentBatchI += 1
			batchMu.Unlock()

			fmt.Printf("Sent batch %d / %d\n", currentBatchI, len(requestBatches))
		}
	}
	fmt.Printf("Successfully sent %d requests\n", numRequestsSent)
	return nil
}
