package ingest

import (
	"context"
	"errors"
	"testing"
	"time"

	supportlog "github.com/stellar/go/support/log"
	"github.com/stretchr/testify/assert"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

type ErrorReadWriter struct {
}

func (rw *ErrorReadWriter) GetLatestLedgerSequence(ctx context.Context) (uint32, error) {
	return 0, errors.New("could not get latest ledger sequence")
}
func (rw *ErrorReadWriter) NewTx(ctx context.Context) (db.WriteTx, error) {
	return nil, errors.New("could not create new tx")
}

func TestRetryRunningIngestion(t *testing.T) {

	numRetries := 0
	var lastErr error
	incrementRetry := func(err error, dur time.Duration) {
		numRetries++
		lastErr = err
	}
	config := Config{
		Logger:            supportlog.New(),
		DB:                &ErrorReadWriter{},
		EventStore:        nil,
		TransactionStore:  nil,
		NetworkPassPhrase: "",
		Archive:           nil,
		LedgerBackend:     nil,
		Timeout:           time.Second,
		OnIngestionRetry:  incrementRetry,
	}
	service := NewService(config)
	time.Sleep(50 * time.Millisecond)
	service.Close()
	assert.Equal(t, 1, numRetries)
	assert.Error(t, lastErr)
	assert.ErrorContains(t, lastErr, "could not get latest ledger sequence")
}
