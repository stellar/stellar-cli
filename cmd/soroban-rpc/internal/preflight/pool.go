package preflight

import (
	"context"
	"errors"
	"sync"

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
	done              chan struct{}
	requestChan       chan workerRequest
	wg                sync.WaitGroup
}

func NewPreflightWorkerPool(workerCount uint, ledgerEntryReader db.LedgerEntryReader, networkPassphrase string, logger *log.Entry) *PreflightWorkerPool {
	preflightWP := PreflightWorkerPool{
		ledgerEntryReader: ledgerEntryReader,
		networkPassphrase: networkPassphrase,
		logger:            logger,
		done:              make(chan struct{}),
		requestChan:       make(chan workerRequest),
	}
	for i := uint(0); i < workerCount; i++ {
		preflightWP.wg.Add(1)
		go preflightWP.work()
	}
	return &preflightWP
}

func (pwp *PreflightWorkerPool) work() {
	defer pwp.wg.Done()
	for {
		select {
		case <-pwp.done:
			return
		case request := <-pwp.requestChan:
			preflight, err := GetPreflight(request.ctx, request.params)
			request.resultChan <- workerResult{preflight, err}
		}
	}
}

func (pwp *PreflightWorkerPool) Close() {
	close(pwp.done)
	pwp.wg.Wait()
}

func (pwp *PreflightWorkerPool) GetPreflight(ctx context.Context, sourceAccount xdr.AccountId, op xdr.InvokeHostFunctionOp) (Preflight, error) {
	params := PreflightParameters{
		Logger:             pwp.logger,
		SourceAccount:      sourceAccount,
		InvokeHostFunction: op,
		NetworkPassphrase:  pwp.networkPassphrase,
		LedgerEntryReader:  pwp.ledgerEntryReader,
	}
	resultC := make(chan workerResult)
	select {
	case <-pwp.done:
		return Preflight{}, errors.New("preflight worker pool is closed")
	case pwp.requestChan <- workerRequest{ctx, params, resultC}:
		result := <-resultC
		return result.preflight, result.err
	case <-ctx.Done():
		return Preflight{}, ctx.Err()
	}
}
