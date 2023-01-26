package events

import (
	"fmt"
	"io"
	"math"
	"sort"
	"sync"

	"github.com/stellar/go/ingest"
	"github.com/stellar/go/xdr"
)

// Cursor represents the position of an event within the sequence of all
// Soroban events. Soroban events are sorted by ledger sequence, transaction
// index, operation index, and event index.
type Cursor struct {
	// Ledger is the sequence of the ledger which emitted the event.
	Ledger uint32
	// Tx is the index of the transaction within the ledger which emitted the event.
	Tx uint32
	// Op is the index of the operation within the transaction which emitted the event.
	Op uint32
	// Event is the index of the event within in the operation which emitted the event.
	Event uint32
}

func cmp(a, b uint32) int {
	if a < b {
		return -1
	}
	if a > b {
		return 1
	}
	return 0
}

// Cmp compares two cursors.
// 0 is returned if the c is equal to other.
// 1 is returned if c is greater than other.
// -1 is returned if c is less than other.
func (c Cursor) Cmp(other Cursor) int {
	if c.Ledger == other.Ledger {
		if c.Tx == other.Tx {
			if c.Op == other.Op {
				return cmp(c.Event, other.Event)
			}
			return cmp(c.Op, other.Op)
		}
		return cmp(c.Tx, other.Tx)
	}
	return cmp(c.Ledger, other.Ledger)
}

var (
	// MinCursor is the smallest possible cursor
	MinCursor = Cursor{}
	// MaxCursor is the largest possible cursor
	MaxCursor = Cursor{
		Ledger: math.MaxUint32,
		Tx:     math.MaxUint32,
		Op:     math.MaxUint32,
		Event:  math.MaxUint32,
	}
)

type bucket struct {
	ledgerSeq uint32
	events    []event
}

type event struct {
	contents   xdr.ContractEvent
	txIndex    uint32
	opIndex    uint32
	eventIndex uint32
}

func (e event) cursor(ledger uint32) Cursor {
	return Cursor{
		Ledger: ledger,
		Tx:     e.txIndex,
		Op:     e.opIndex,
		Event:  e.eventIndex,
	}
}

// MemoryStore is an in-memory store of soroban events.
type MemoryStore struct {
	lock sync.RWMutex
	// buckets is a circular buffer where each cell represents
	// all events occurring within a specific ledger.
	buckets []bucket
	// length is equal to the number of ledgers contained within
	// the circular buffer.
	length uint32
	// start is the index of the head in the circular buffer.
	start uint32
}

// NewMemoryStore creates a new MemoryStore populated by the given ledgers.
// retentionWindow defines a retention window in ledgers. All events occurring
// within the retention window will be included in the store.
func NewMemoryStore(retentionWindow uint32) (*MemoryStore, error) {
	if retentionWindow == 0 {
		return nil, fmt.Errorf("retention window must be positive")
	}
	return &MemoryStore{
		buckets: make([]bucket, retentionWindow),
	}, nil
}

// Range defines an interval in the sequence of all Soroban events.
type Range struct {
	// Start defines the start of the range.
	// Start is included in the range.
	Start Cursor
	// ClampStart indicates whether Start should be clamped to
	// the earliest ledger available if Start is too low.
	ClampStart bool
	// End defines the end of the range.
	// End is excluded from the range.
	End Cursor
	// ClampEnd indicates whether End should be clamped to
	// the latest ledger available if End is too high.
	ClampEnd bool
}

// Scan applies f on all the events occurring in the given range.
// The events are processed in sorted ascending Cursor order.
// If f returns false, the scan terminates early (f will not be applied on
// remaining events in the range). Note that a read lock is held for the
// entire duration of the Scan function so f be written in a way
// to minimize latency.
func (m *MemoryStore) Scan(eventRange Range, f func(Cursor, xdr.ContractEvent) bool) error {
	m.lock.RLock()
	defer m.lock.RUnlock()

	if err := m.validateRange(&eventRange); err != nil {
		return err
	}

	curLedger := eventRange.Start.Ledger
	minLedger := m.buckets[m.start].ledgerSeq
	i := ((curLedger - minLedger) + m.start) % uint32(len(m.buckets))
	events := seek(m.buckets[i].events, eventRange.Start)
	for {
		for _, event := range events {
			cur := event.cursor(curLedger)
			if eventRange.End.Cmp(cur) <= 0 {
				return nil
			}
			if !f(cur, event.contents) {
				return nil
			}
		}
		i = (i + 1) % uint32(len(m.buckets))
		curLedger++
		if m.buckets[i].ledgerSeq != curLedger {
			return nil
		}
		events = m.buckets[i].events
	}
}

// validateRange checks if the range falls within the bounds
// of the events in the memory store.
// validateRange should be called with the read lock.
func (m *MemoryStore) validateRange(eventRange *Range) error {
	if m.length == 0 {
		return fmt.Errorf("event store is empty")
	}

	min := Cursor{Ledger: m.buckets[m.start].ledgerSeq}
	if eventRange.Start.Cmp(min) < 0 {
		if eventRange.ClampStart {
			eventRange.Start = min
		} else {
			return fmt.Errorf("start is before oldest ledger")
		}
	}

	max := Cursor{Ledger: min.Ledger + m.length}
	if eventRange.End.Cmp(max) > 0 {
		if eventRange.ClampEnd {
			eventRange.End = max
		} else {
			return fmt.Errorf("end is after latest ledger")
		}
	}

	if eventRange.Start.Cmp(eventRange.End) >= 0 {
		return fmt.Errorf("start is not before end")
	}

	return nil
}

// seek returns the subset of all events which occur
// at a point greater than or equal to the given cursor.
// events must be sorted in ascending order.
func seek(events []event, cursor Cursor) []event {
	i := sort.Search(len(events), func(i int) bool {
		event := events[i]
		return cursor.Cmp(event.cursor(cursor.Ledger)) <= 0
	})
	return events[i:]
}

// IngestEvents adds new events from the given ledger into the store.
// As a side effect, events which fall outside the retention window are
// removed from the store.
func (m *MemoryStore) IngestEvents(txReader *ingest.LedgerTransactionReader) error {
	events, err := readEvents(txReader)
	if err != nil {
		return err
	}
	ledgerSequence := txReader.GetSequence()
	return m.append(ledgerSequence, events)
}

func readEvents(txReader *ingest.LedgerTransactionReader) ([]event, error) {
	var events []event
	for {
		tx, err := txReader.Read()
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, err
		}

		for i := range tx.Envelope.Operations() {
			opIndex := uint32(i)
			opEvents, err := tx.GetOperationEvents(opIndex)
			if err != nil {
				return nil, err
			}
			for eventIndex, opEvent := range opEvents {
				events = append(events, event{
					contents:   opEvent,
					txIndex:    tx.Index,
					opIndex:    opIndex,
					eventIndex: uint32(eventIndex),
				})
			}
		}
	}
	return events, nil
}

// append adds new events to the circular buffer.
func (m *MemoryStore) append(sequence uint32, events []event) error {
	m.lock.Lock()
	defer m.lock.Unlock()

	expectedLedgerSequence := m.buckets[m.start].ledgerSeq + m.length
	if m.length > 0 && expectedLedgerSequence != sequence {
		return fmt.Errorf("events not contiguous: expected ledger sequence %v but received %v", expectedLedgerSequence, sequence)
	}

	index := (m.start + m.length) % uint32(len(m.buckets))
	m.buckets[index] = bucket{
		ledgerSeq: sequence,
		events:    events,
	}
	if m.length < uint32(len(m.buckets)) {
		m.length++
	} else {
		m.start++
	}

	return nil
}
