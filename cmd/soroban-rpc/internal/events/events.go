package events

import (
	"errors"
	"io"
	"sort"
	"sync"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/ingest"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
)

type event struct {
	diagnosticEventXDR []byte
	txIndex            uint32
	eventIndex         uint32
	txHash     *xdr.Hash // intentionally stored as a pointer to save memory (amortized as soon as there are two events in a transaction)
}

func (e event) cursor(ledgerSeq uint32) Cursor {
	return Cursor{
		Ledger: ledgerSeq,
		Tx:     e.txIndex,
		Event:  e.eventIndex,
	}
}

// MemoryStore is an in-memory store of soroban events.
type MemoryStore struct {
	// networkPassphrase is an immutable string containing the
	// Stellar network passphrase.
	// Accessing networkPassphrase does not need to be protected
	// by the lock
	networkPassphrase string
	// lock protects the mutable fields below
	lock                 sync.RWMutex
	eventsByLedger       *ledgerbucketwindow.LedgerBucketWindow[[]event]
	eventsDurationMetric *prometheus.SummaryVec
	eventCountMetric     prometheus.Summary
}

// NewMemoryStore creates a new MemoryStore.
// The retention window is in units of ledgers.
// All events occurring in the following ledger range
// [ latestLedger - retentionWindow, latestLedger ]
// will be included in the MemoryStore. If the MemoryStore
// is full, any events from new ledgers will evict
// older entries outside the retention window.
func NewMemoryStore(daemon interfaces.Daemon, networkPassphrase string, retentionWindow uint32) *MemoryStore {
	window := ledgerbucketwindow.NewLedgerBucketWindow[[]event](retentionWindow)

	// eventsDurationMetric is a metric for measuring latency of event store operations
	eventsDurationMetric := prometheus.NewSummaryVec(prometheus.SummaryOpts{
		Namespace: daemon.MetricsNamespace(), Subsystem: "events", Name: "operation_duration_seconds",
		Help:       "event store operation durations, sliding window = 10m",
		Objectives: map[float64]float64{0.5: 0.05, 0.9: 0.01, 0.99: 0.001},
	},
		[]string{"operation"},
	)

	eventCountMetric := prometheus.NewSummary(prometheus.SummaryOpts{
		Namespace: daemon.MetricsNamespace(), Subsystem: "events", Name: "count",
		Help:       "count of events ingested, sliding window = 10m",
		Objectives: map[float64]float64{0.5: 0.05, 0.9: 0.01, 0.99: 0.001},
	})
	daemon.MetricsRegistry().MustRegister(eventCountMetric, eventsDurationMetric)
	return &MemoryStore{
		networkPassphrase:    networkPassphrase,
		eventsByLedger:       window,
		eventsDurationMetric: eventsDurationMetric,
		eventCountMetric:     eventCountMetric,
	}
}

// Range defines a [Start, End) interval of Soroban events.
type Range struct {
	// Start defines the (inclusive) start of the range.
	Start Cursor
	// ClampStart indicates whether Start should be clamped up
	// to the earliest ledger available if Start is too low.
	ClampStart bool
	// End defines the (exclusive) end of the range.
	End Cursor
	// ClampEnd indicates whether End should be clamped down
	// to the latest ledger available if End is too high.
	ClampEnd bool
}

type ScanFunction func(xdr.DiagnosticEvent, Cursor, int64, *xdr.Hash) bool

// Scan applies f on all the events occurring in the given range.
// The events are processed in sorted ascending Cursor order.
// If f returns false, the scan terminates early (f will not be applied on
// remaining events in the range). Note that a read lock is held for the
// entire duration of the Scan function so f should be written in a way
// to minimize latency.
func (m *MemoryStore) Scan(eventRange Range, f ScanFunction) (uint32, error) {
	startTime := time.Now()
	m.lock.RLock()
	defer m.lock.RUnlock()

	if err := m.validateRange(&eventRange); err != nil {
		return 0, err
	}

	firstLedgerInRange := eventRange.Start.Ledger
	firstLedgerInWindow := m.eventsByLedger.Get(0).LedgerSeq
	lastLedgerInWindow := firstLedgerInWindow + (m.eventsByLedger.Len() - 1)
	for i := firstLedgerInRange - firstLedgerInWindow; i < m.eventsByLedger.Len(); i++ {
		bucket := m.eventsByLedger.Get(i)
		events := bucket.BucketContent
		if bucket.LedgerSeq == firstLedgerInRange {
			// we need to seek for the beginning of the events in the first bucket in the range
			events = seek(events, eventRange.Start)
		}
		timestamp := bucket.LedgerCloseTimestamp
		for _, event := range events {
			cur := event.cursor(bucket.LedgerSeq)
			if eventRange.End.Cmp(cur) <= 0 {
				return lastLedgerInWindow, nil
			}
			var diagnosticEvent xdr.DiagnosticEvent
			err := xdr.SafeUnmarshal(event.diagnosticEventXDR, &diagnosticEvent)
			if err != nil {
				return 0, err
			}
			if !f(diagnosticEvent, cur, timestamp, event.txHash) {
				return lastLedgerInWindow, nil
			}
		}
	}
	m.eventsDurationMetric.With(prometheus.Labels{"operation": "scan"}).
		Observe(time.Since(startTime).Seconds())
	return lastLedgerInWindow, nil
}

// validateRange checks if the range falls within the bounds
// of the events in the memory store.
// validateRange should be called with the read lock.
func (m *MemoryStore) validateRange(eventRange *Range) error {
	if m.eventsByLedger.Len() == 0 {
		return errors.New("event store is empty")
	}
	firstBucket := m.eventsByLedger.Get(0)
	min := Cursor{Ledger: firstBucket.LedgerSeq}
	if eventRange.Start.Cmp(min) < 0 {
		if eventRange.ClampStart {
			eventRange.Start = min
		} else {
			return errors.New("start is before oldest ledger")
		}
	}
	max := Cursor{Ledger: min.Ledger + m.eventsByLedger.Len()}
	if eventRange.Start.Cmp(max) >= 0 {
		return errors.New("start is after newest ledger")
	}
	if eventRange.End.Cmp(max) > 0 {
		if eventRange.ClampEnd {
			eventRange.End = max
		} else {
			return errors.New("end is after latest ledger")
		}
	}

	if eventRange.Start.Cmp(eventRange.End) >= 0 {
		return errors.New("start is not before end")
	}

	return nil
}

// seek returns the subset of all events which occur
// at a point greater than or equal to the given cursor.
// events must be sorted in ascending order.
func seek(events []event, cursor Cursor) []event {
	j := sort.Search(len(events), func(i int) bool {
		return cursor.Cmp(events[i].cursor(cursor.Ledger)) <= 0
	})
	return events[j:]
}

// IngestEvents adds new events from the given ledger into the store.
// As a side effect, events which fall outside the retention window are
// removed from the store.
func (m *MemoryStore) IngestEvents(ledgerCloseMeta xdr.LedgerCloseMeta) error {
	startTime := time.Now()
	// no need to acquire the lock because the networkPassphrase field
	// is immutable
	events, err := readEvents(m.networkPassphrase, ledgerCloseMeta)
	if err != nil {
		return err
	}
	bucket := ledgerbucketwindow.LedgerBucket[[]event]{
		LedgerSeq:            ledgerCloseMeta.LedgerSequence(),
		LedgerCloseTimestamp: int64(ledgerCloseMeta.LedgerHeaderHistoryEntry().Header.ScpValue.CloseTime),
		BucketContent:        events,
	}
	m.lock.Lock()
	if _, err = m.eventsByLedger.Append(bucket); err != nil {
		return err
	}
	m.lock.Unlock()
	m.eventsDurationMetric.With(prometheus.Labels{"operation": "ingest"}).
		Observe(time.Since(startTime).Seconds())
	m.eventCountMetric.Observe(float64(len(events)))
	return nil
}

func readEvents(networkPassphrase string, ledgerCloseMeta xdr.LedgerCloseMeta) (events []event, err error) {
	var txReader *ingest.LedgerTransactionReader
	txReader, err = ingest.NewLedgerTransactionReaderFromLedgerCloseMeta(networkPassphrase, ledgerCloseMeta)
	if err != nil {
		return
	}
	defer func() {
		closeErr := txReader.Close()
		if err == nil {
			err = closeErr
		}
	}()

	for {
		var tx ingest.LedgerTransaction
		tx, err = txReader.Read()
		if err == io.EOF {
			err = nil
			break
		}
		if err != nil {
			return
		}

		if !tx.Result.Successful() {
			continue
		}

		txEvents, err := tx.GetDiagnosticEvents()
		if err != nil {
			return nil, err
		}
		txHash := tx.Result.TransactionHash
		for index, e := range txEvents {
			diagnosticEventXDR, err := e.MarshalBinary()
			if err != nil {
				return nil, err
			}
			events = append(events, event{
				diagnosticEventXDR: diagnosticEventXDR,
				txIndex:            tx.Index,
				eventIndex:         uint32(index),
				txHash:             &txHash,
			})
		}
	}
	return events, err
}
