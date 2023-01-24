package events

import (
	"io"
	"math"
	"sort"
	"sync"

	"github.com/stellar/go/ingest"
	"github.com/stellar/go/xdr"
)

// Cursor represents a specific time when an event was emitted.
type Cursor struct {
	// Ledger is the sequence of the ledger which emitted the event.
	Ledger uint32
	// Tx is the transaction index within the ledger which emitted the event.
	Tx uint32
	// Op is the operation index within the transaction which emitted the event.
	Op uint32
	// Event is the index within all events occurring in the operation which emitted the event.
	Event uint32
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

type event struct {
	contents xdr.ContractEvent
	cursor   Cursor
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
			return cmp(c.Event, other.Event)
		}
		return cmp(c.Tx, other.Tx)
	}
	return cmp(c.Ledger, other.Ledger)
}

// MemoryStore is an in-memory store of soroban events.
type MemoryStore struct {
	lock            sync.RWMutex
	events          []event
	length          int
	start           int
	retentionWindow uint32
}

// NewMemoryStore creates a new MemoryStore populated by the given ledgers.
func NewMemoryStore(passPhrase string, ledgers []xdr.LedgerCloseMeta, retentionWindow uint32) (*MemoryStore, error) {
	var events []event
	for _, ledger := range ledgers {
		reader, err := ingest.NewLedgerTransactionReaderFromLedgerCloseMeta(passPhrase, ledger)
		if err != nil {
			return nil, err
		}
		ledgerEvents, err := readEvents(reader)
		if err != nil {
			return nil, err
		}
		events = append(events, ledgerEvents...)
	}

	return &MemoryStore{
		events:          events,
		retentionWindow: retentionWindow,
		length:          len(events),
	}, nil
}

// Scan calls f on all the events in the interval of [start, end).
// If f returns false, the scan terminates early and no more events are applied on f.
func (m *MemoryStore) Scan(start, end Cursor, f func(Cursor, xdr.ContractEvent) bool) {
	m.lock.RLock()
	defer m.lock.RUnlock()

	i := sort.Search(m.length, func(i int) bool {
		index := (m.start + i) % m.length
		entry := m.events[index]
		return entry.cursor.Cmp(start) < 0
	}) + 1

	for ; i < m.length; i++ {
		index := (m.start + i) % m.length
		entry := m.events[index]
		if entry.cursor.Cmp(end) >= 0 {
			break
		}
		if !f(entry.cursor, entry.contents) {
			break
		}
	}
}

// IngestEvents adds new events from the given ledger into the store.
func (m *MemoryStore) IngestEvents(txReader *ingest.LedgerTransactionReader) error {
	events, err := readEvents(txReader)
	if err != nil {
		return err
	}
	if len(events) == 0 {
		return nil
	}

	m.lock.Lock()
	defer m.lock.Unlock()

	m.trim(txReader.GetSequence())
	m.maybeResize(len(events))
	m.append(events)
	return nil
}

func readEvents(txReader *ingest.LedgerTransactionReader) ([]event, error) {
	var events []event
	sequence := txReader.GetSequence()
	for {
		tx, err := txReader.Read()
		if err == io.EOF {
			break
		}
		if err != nil {
			return nil, err
		}
		// TODO : add function in ingest.LedgerTransaction to obtain operation events
		txMeta, ok := tx.UnsafeMeta.GetV3()
		if !ok {
			continue
		}
		for opIndex, op := range txMeta.Events {
			if len(op.Events) == 0 {
				continue
			}
			for eventIndex, opEvent := range op.Events {
				events = append(events, event{
					contents: opEvent,
					cursor: Cursor{
						Ledger: sequence,
						Tx:     tx.Index,
						Op:     uint32(opIndex),
						Event:  uint32(eventIndex),
					},
				})
			}
		}
	}
	return events, nil
}

func (m *MemoryStore) trim(latestSequence uint32) {
	if latestSequence+1 <= m.retentionWindow {
		return
	}
	cutoff := latestSequence + 1 - m.retentionWindow
	start := m.start
	end := start + m.length
	for cur := start; cur < end; cur++ {
		cur = cur % len(m.events)
		if m.events[cur].cursor.Ledger < cutoff {
			m.start++
			m.length--
		}
	}
}

func (m *MemoryStore) maybeResize(extraRequiredCapacity int) {
	if len(m.events) >= m.length+extraRequiredCapacity {
		return
	}
	minSize := m.length + extraRequiredCapacity
	// scale new buffer by 120%
	resized := make([]event, minSize+minSize/5)
	for i := 0; i < m.length; i++ {
		resized[i] = m.events[(m.start+i)%m.length]
	}
	m.events = resized
	m.start = 0
}

func (m *MemoryStore) append(events []event) {
	start := m.start + m.length
	for i, event := range events {
		index := (i + start) % len(m.events)
		m.events[index] = event
	}
	m.length += len(events)
}
