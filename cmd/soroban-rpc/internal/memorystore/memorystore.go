package memorystore

import (
	"errors"
	"fmt"
	"sync"

	"github.com/stellar/go/xdr"
)

type bucket struct {
	ledgerSeq            uint32
	ledgerCloseTimestamp int64
	events               []event
	// transactions in the memory store belonging to the ledger in the current bucket
	// this is used for garbage-collecting the global memory store transactions when the bucket is evicted
	transactionHashes []xdr.Hash
}

// MemoryStore is an in-memory store of soroban events.
type MemoryStore struct {
	// networkPassphrase is an immutable string containing the
	// Stellar network passphrase.
	// Accessing networkPassphrase does not need to be protected
	// by the lock
	networkPassphrase string
	// lock protects the mutable fields below
	lock sync.RWMutex
	// buckets is a circular buffer where each cell represents
	// all events occurring within a specific ledger.
	buckets []bucket
	// start is the index of the head in the circular buffer.
	start        uint32
	transactions map[xdr.Hash]Transaction
}

// NewMemoryStore creates a new MemoryStore.
// The retention window is in units of ledgers.
// All events occurring in the following ledger range
// [ latestLedger - retentionWindow, latestLedger ]
// will be included in the MemoryStore. If the MemoryStore
// is full, any events from new ledgers will evict
// older entries outside the retention window.
func NewMemoryStore(networkPassphrase string, retentionWindow uint32) (*MemoryStore, error) {
	if retentionWindow == 0 {
		return nil, errors.New("retention window must be positive")
	}
	return &MemoryStore{
		networkPassphrase: networkPassphrase,
		buckets:           make([]bucket, 0, retentionWindow),
	}, nil
}

// EventRange defines a [Start, End) interval of Soroban events.
type EventRange struct {
	// Start defines the (inclusive) start of the range.
	Start EventCursor
	// ClampStart indicates whether Start should be clamped up
	// to the earliest ledger available if Start is too low.
	ClampStart bool
	// End defines the (exclusive) end of the range.
	End EventCursor
	// ClampEnd indicates whether End should be clamped down
	// to the latest ledger available if End is too high.
	ClampEnd bool
}

// ScanEvents applies f on all the events occurring in the given range.
// The events are processed in sorted ascending EventCursor order.
// If f returns false, the scan terminates early (f will not be applied on
// remaining events in the range). Note that a read lock is held for the
// entire duration of the Scan function so f should be written in a way
// to minimize latency.
func (m *MemoryStore) ScanEvents(eventRange EventRange, f func(xdr.ContractEvent, EventCursor, int64) bool) (uint32, error) {
	m.lock.RLock()
	defer m.lock.RUnlock()

	if err := m.validateEventRange(&eventRange); err != nil {
		return 0, err
	}

	curLedger := eventRange.Start.Ledger
	minLedger := m.buckets[m.start].ledgerSeq
	latestLedger := minLedger + uint32(len(m.buckets))
	i := ((curLedger - minLedger) + m.start) % uint32(len(m.buckets))
	events := seekEvents(m.buckets[i].events, eventRange.Start)
	for ; curLedger == m.buckets[i].ledgerSeq; curLedger++ {
		timestamp := m.buckets[i].ledgerCloseTimestamp
		for _, event := range events {
			cur := event.cursor(curLedger)
			if eventRange.End.Cmp(cur) <= 0 {
				return latestLedger, nil
			}
			if !f(event.contents, cur, timestamp) {
				return latestLedger, nil
			}
		}
		i = (i + 1) % uint32(len(m.buckets))
		events = m.buckets[i].events
	}
	return latestLedger, nil
}

// validateEventRange checks if the range falls within the bounds
// of the events in the memory store.
// validateEventRange should be called with the read lock.
func (m *MemoryStore) validateEventRange(eventRange *EventRange) error {
	if len(m.buckets) == 0 {
		return errors.New("event store is empty")
	}

	min := EventCursor{Ledger: m.buckets[m.start].ledgerSeq}
	if eventRange.Start.Cmp(min) < 0 {
		if eventRange.ClampStart {
			eventRange.Start = min
		} else {
			return errors.New("start is before oldest ledger")
		}
	}
	max := EventCursor{Ledger: min.Ledger + uint32(len(m.buckets))}
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

// Ingest adds new data from the given ledger into the store.
// As a side effect, data which falls outside the retention window are
// removed from the store.
func (m *MemoryStore) Ingest(ledgerCloseMeta xdr.LedgerCloseMeta) error {
	// no need to acquire the lock because the networkPassphrase field
	// is immutable
	events, err := readEvents(m.networkPassphrase, ledgerCloseMeta)
	if err != nil {
		return err
	}
	transactions := readTransactions(ledgerCloseMeta)
	ledgerCloseMeta.TransactionEnvelopes()
	ledgerSequence := ledgerCloseMeta.LedgerSequence()
	ledgerCloseTime := int64(ledgerCloseMeta.LedgerHeaderHistoryEntry().Header.ScpValue.CloseTime)
	return m.append(ledgerSequence, ledgerCloseTime, events, transactions)
}

// append adds new events to the circular buffer.
func (m *MemoryStore) append(sequence uint32, ledgerCloseTimestamp int64, events []event, transactions []Transaction) error {
	m.lock.Lock()
	defer m.lock.Unlock()

	length := uint32(len(m.buckets))
	if length > 0 {
		expectedLedgerSequence := m.buckets[m.start].ledgerSeq + length
		if expectedLedgerSequence != sequence {
			return fmt.Errorf("events not contiguous: expected ledger sequence %v but received %v", expectedLedgerSequence, sequence)
		}
	}
	transactionHashes := make([]xdr.Hash, len(transactions))
	for i := range transactions {
		transactionHashes[i] = transactions[i].id
	}

	nextBucket := bucket{
		ledgerCloseTimestamp: ledgerCloseTimestamp,
		ledgerSeq:            sequence,
		events:               events,
		transactionHashes:    transactionHashes,
	}
	if length < uint32(cap(m.buckets)) {
		m.buckets = append(m.buckets, nextBucket)
	} else {
		index := (m.start + length) % uint32(len(m.buckets))
		// garbage-collect the transactions from the bucket we are evicting
		for _, hash := range m.buckets[index].transactionHashes {
			delete(m.transactions, hash)
		}
		m.buckets[index] = nextBucket
		m.start++
	}

	return nil
}
