package preflight

import (
	"context"
	"errors"
	"sync"
	"sync/atomic"

	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

type workerResult struct {
	preflight Preflight
	err       error
}

type workerRequest struct {
	ctx        context.Context
	params     PreflightParameters
	resultChan chan<- workerResult
}

type PreflightWorkerPool struct {
	ledgerEntryReader db.LedgerEntryReader
	networkPassphrase string
	logger            *log.Entry
	isClosed          atomic.Bool
	requestChan       chan workerRequest
	wg                sync.WaitGroup
}

func NewPreflightWorkerPool(workerCount uint, jobQueueCapacity uint, ledgerEntryReader db.LedgerEntryReader, networkPassphrase string, logger *log.Entry) *PreflightWorkerPool {
	preflightWP := PreflightWorkerPool{
		ledgerEntryReader: ledgerEntryReader,
		networkPassphrase: networkPassphrase,
		logger:            logger,
		requestChan:       make(chan workerRequest, jobQueueCapacity),
	}
	for i := uint(0); i < workerCount; i++ {
		preflightWP.wg.Add(1)
		go preflightWP.work()
	}
	return &preflightWP
}

func (pwp *PreflightWorkerPool) work() {
	defer pwp.wg.Done()
	for request := range pwp.requestChan {
		preflight, err := GetPreflight(request.ctx, request.params)
		request.resultChan <- workerResult{preflight, err}
	}
}

func (pwp *PreflightWorkerPool) Close() {
	if !pwp.isClosed.CompareAndSwap(false, true) {
		// it was already closed
		return
	}
	close(pwp.requestChan)
	pwp.wg.Wait()
}

var PreflightQueueFullErr = errors.New("preflight queue full")

func (pwp *PreflightWorkerPool) GetPreflight(ctx context.Context, readTx db.LedgerEntryReadTx, sourceAccount xdr.AccountId, op xdr.InvokeHostFunctionOp) (Preflight, error) {
	if pwp.isClosed.Load() {
		return Preflight{}, errors.New("preflight worker pool is closed")
	}
	params := PreflightParameters{
		Logger:             pwp.logger,
		SourceAccount:      sourceAccount,
		InvokeHostFunction: op,
		NetworkPassphrase:  pwp.networkPassphrase,
		LedgerEntryReadTx:  readTx,
	}
	resultC := make(chan workerResult)
	select {
	case pwp.requestChan <- workerRequest{ctx, params, resultC}:
		result := <-resultC
		return result.preflight, result.err
	case <-ctx.Done():
		return Preflight{}, ctx.Err()
	default:
		return Preflight{}, PreflightQueueFullErr
	}
}
