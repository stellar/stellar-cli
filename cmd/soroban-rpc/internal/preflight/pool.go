package preflight

import (
	"context"
	"errors"
	"sync"
	"sync/atomic"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
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
	ledgerEntryReader          db.LedgerEntryReader
	networkPassphrase          string
	logger                     *log.Entry
	isClosed                   atomic.Bool
	requestChan                chan workerRequest
	concurrentRequestsMetric   prometheus.Gauge
	errorFullCounter           prometheus.Counter
	durationMetric             *prometheus.SummaryVec
	ledgerEntriesFetchedMetric prometheus.Summary
	wg                         sync.WaitGroup
}

func NewPreflightWorkerPool(daemon interfaces.Daemon, workerCount uint, jobQueueCapacity uint, ledgerEntryReader db.LedgerEntryReader, networkPassphrase string, logger *log.Entry) *PreflightWorkerPool {
	preflightWP := PreflightWorkerPool{
		ledgerEntryReader: ledgerEntryReader,
		networkPassphrase: networkPassphrase,
		logger:            logger,
		requestChan:       make(chan workerRequest, jobQueueCapacity),
	}
	requestQueueMetric := prometheus.NewGaugeFunc(prometheus.GaugeOpts{
		Namespace: daemon.MetricsNamespace(),
		Subsystem: "preflight_pool",
		Name:      "queue_length",
		Help:      "number of preflight requests in the queue",
	}, func() float64 {
		return float64(len(preflightWP.requestChan))
	})
	preflightWP.concurrentRequestsMetric = prometheus.NewGauge(prometheus.GaugeOpts{
		Namespace: daemon.MetricsNamespace(),
		Subsystem: "preflight_pool",
		Name:      "concurrent_requests",
		Help:      "number of preflight requests currently running",
	})
	preflightWP.errorFullCounter = prometheus.NewCounter(prometheus.CounterOpts{
		Namespace: daemon.MetricsNamespace(),
		Subsystem: "preflight_pool",
		Name:      "queue_full_errors",
		Help:      "number of preflight full queue errors",
	})
	preflightWP.durationMetric = prometheus.NewSummaryVec(prometheus.SummaryOpts{
		Namespace: daemon.MetricsNamespace(),
		Subsystem: "preflight_pool",
		Name:      "request_ledger_get_duration_seconds",
		Help:      "preflight request duration broken down by status",
	}, []string{"status", "type"})
	preflightWP.ledgerEntriesFetchedMetric = prometheus.NewSummary(prometheus.SummaryOpts{
		Namespace: daemon.MetricsNamespace(),
		Subsystem: "preflight_pool",
		Name:      "request_ledger_entries_fetched",
		Help:      "ledger entries fetched by simulate transaction calls",
	})
	daemon.MetricsRegistry().MustRegister(
		requestQueueMetric,
		preflightWP.concurrentRequestsMetric,
		preflightWP.errorFullCounter,
		preflightWP.durationMetric,
		preflightWP.ledgerEntriesFetchedMetric,
	)
	for i := uint(0); i < workerCount; i++ {
		preflightWP.wg.Add(1)
		go preflightWP.work()
	}
	return &preflightWP
}

func (pwp *PreflightWorkerPool) work() {
	defer pwp.wg.Done()
	for request := range pwp.requestChan {
		pwp.concurrentRequestsMetric.Inc()
		startTime := time.Now()
		preflight, err := GetPreflight(request.ctx, request.params)
		status := "ok"
		if err != nil {
			status = "error"
		}
		pwp.durationMetric.With(
			prometheus.Labels{"type": "all", "status": status},
		).Observe(time.Since(startTime).Seconds())
		pwp.concurrentRequestsMetric.Dec()
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

type metricsLedgerEntryWrapper struct {
	db.LedgerEntryReadTx
	totalDurationMs      uint64
	ledgerEntriesFetched uint32
}

func (m *metricsLedgerEntryWrapper) GetLedgerEntry(key xdr.LedgerKey, includeExpired bool) (bool, xdr.LedgerEntry, error) {
	startTime := time.Now()
	ok, entry, err := m.LedgerEntryReadTx.GetLedgerEntry(key, includeExpired)
	atomic.AddUint64(&m.totalDurationMs, uint64(time.Since(startTime).Milliseconds()))
	atomic.AddUint32(&m.ledgerEntriesFetched, 1)
	return ok, entry, err
}

func (pwp *PreflightWorkerPool) GetPreflight(ctx context.Context, readTx db.LedgerEntryReadTx, sourceAccount xdr.AccountId, opBody xdr.OperationBody, footprint xdr.LedgerFootprint) (Preflight, error) {
	if pwp.isClosed.Load() {
		return Preflight{}, errors.New("preflight worker pool is closed")
	}
	wrappedTx := metricsLedgerEntryWrapper{
		LedgerEntryReadTx: readTx,
	}
	params := PreflightParameters{
		Logger:            pwp.logger,
		SourceAccount:     sourceAccount,
		OpBody:            opBody,
		NetworkPassphrase: pwp.networkPassphrase,
		LedgerEntryReadTx: &wrappedTx,
		Footprint:         footprint,
	}
	resultC := make(chan workerResult)
	select {
	case pwp.requestChan <- workerRequest{ctx, params, resultC}:
		result := <-resultC
		if wrappedTx.ledgerEntriesFetched > 0 {
			status := "ok"
			if result.err != nil {
				status = "error"
			}
			pwp.durationMetric.With(
				prometheus.Labels{"type": "db", "status": status},
			).Observe(float64(wrappedTx.totalDurationMs) / 1000.0)
		}
		pwp.ledgerEntriesFetchedMetric.Observe(float64(wrappedTx.ledgerEntriesFetched))
		return result.preflight, result.err
	case <-ctx.Done():
		return Preflight{}, ctx.Err()
	default:
		pwp.errorFullCounter.Inc()
		return Preflight{}, PreflightQueueFullErr
	}
}
