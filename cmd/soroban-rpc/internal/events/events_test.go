package events

import (
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/stellar/go/xdr"
)

var (
	ledger5CloseTime = ledgerCloseTime(5)
	ledger5Events    = []event{
		newEvent(1, 0, 0, 100),
		newEvent(1, 0, 1, 200),
		newEvent(2, 0, 0, 300),
		newEvent(2, 1, 0, 400),
	}
	ledger6CloseTime         = ledgerCloseTime(6)
	ledger6Events    []event = nil
	ledger7CloseTime         = ledgerCloseTime(7)
	ledger7Events            = []event{
		newEvent(1, 0, 0, 500),
	}
	ledger8CloseTime = ledgerCloseTime(8)
	ledger8Events    = []event{
		newEvent(1, 0, 0, 600),
		newEvent(2, 0, 0, 700),
		newEvent(2, 0, 1, 800),
		newEvent(2, 0, 2, 900),
		newEvent(2, 1, 0, 1000),
	}
	ledger9CloseTime = ledgerCloseTime(9)
	ledger9Events    = []event{
		newEvent(1, 0, 0, 1100),
	}
)

func ledgerCloseTime(seq uint32) int64 {
	return int64(seq)*25 + 100
}

func newEvent(txIndex, opIndex, eventIndex, val uint32) event {
	v := xdr.Uint32(val)
	return event{
		contents: xdr.ContractEvent{
			Type: xdr.ContractEventTypeSystem,
			Body: xdr.ContractEventBody{
				V: 0,
				V0: &xdr.ContractEventV0{
					Data: xdr.ScVal{
						Type: xdr.ScValTypeScvU32,
						U32:  &v,
					},
				},
			},
		},
		txIndex:    txIndex,
		opIndex:    opIndex,
		eventIndex: eventIndex,
	}
}

func mustMarshal(e xdr.ContractEvent) string {
	result, err := xdr.MarshalBase64(e)
	if err != nil {
		panic(err)
	}
	return result
}

func (e event) equals(other event) bool {
	return e.txIndex == other.txIndex &&
		e.opIndex == other.opIndex &&
		e.eventIndex == other.eventIndex &&
		mustMarshal(e.contents) == mustMarshal(other.contents)
}

func eventsAreEqual(t *testing.T, a, b []event) {
	require.Equal(t, len(a), len(b))
	for i := range a {
		require.True(t, a[i].equals(b[i]))
	}
}

func TestAppend(t *testing.T) {
	m, err := NewMemoryStore(3)
	require.NoError(t, err)

	// test appending first bucket of events
	require.NoError(t, m.append(5, ledger5CloseTime, ledger5Events))
	require.Equal(t, uint32(5), m.buckets[m.start].ledgerSeq)
	require.Equal(t, ledger5CloseTime, m.buckets[m.start].ledgerCloseTimestamp)
	eventsAreEqual(t, ledger5Events, m.buckets[m.start].events)
	require.Equal(t, 1, len(m.buckets))

	// the next bucket of events must follow the previous bucket (ledger 5)
	require.EqualError(
		t, m.append(10, 100, ledger5Events),
		"events not contiguous: expected ledger sequence 6 but received 10",
	)
	require.EqualError(
		t, m.append(4, 100, ledger5Events),
		"events not contiguous: expected ledger sequence 6 but received 4",
	)
	require.EqualError(
		t, m.append(5, 100, nil),
		"events not contiguous: expected ledger sequence 6 but received 5",
	)
	// check that none of the calls above modified our buckets
	require.Equal(t, ledger5Events, m.buckets[m.start].events)
	require.Equal(t, 1, len(m.buckets))

	// append ledger 6 events, now we have two buckets filled
	require.NoError(t, m.append(6, ledger6CloseTime, ledger6Events))
	eventsAreEqual(t, ledger5Events, m.buckets[m.start].events)
	eventsAreEqual(t, ledger6Events, m.buckets[(m.start+1)%uint32(len(m.buckets))].events)
	require.Equal(t, uint32(6), m.buckets[(m.start+1)%uint32(len(m.buckets))].ledgerSeq)
	require.Equal(t, ledger6CloseTime, m.buckets[(m.start+1)%uint32(len(m.buckets))].ledgerCloseTimestamp)
	require.Equal(t, 2, len(m.buckets))

	// the next bucket of events must follow the previous bucket (ledger 6)
	require.EqualError(
		t, m.append(10, 100, ledger5Events),
		"events not contiguous: expected ledger sequence 7 but received 10",
	)
	require.EqualError(
		t, m.append(5, 100, ledger5Events),
		"events not contiguous: expected ledger sequence 7 but received 5",
	)
	require.EqualError(
		t, m.append(6, 100, nil),
		"events not contiguous: expected ledger sequence 7 but received 6",
	)

	// append ledger 7 events, now we have all three buckets filled
	require.NoError(t, m.append(7, ledger7CloseTime, ledger7Events))
	eventsAreEqual(t, ledger5Events, m.buckets[m.start].events)
	eventsAreEqual(t, ledger6Events, m.buckets[(m.start+1)%uint32(len(m.buckets))].events)
	eventsAreEqual(t, ledger7Events, m.buckets[(m.start+2)%uint32(len(m.buckets))].events)
	require.Equal(t, uint32(7), m.buckets[(m.start+2)%uint32(len(m.buckets))].ledgerSeq)
	require.Equal(t, ledger7CloseTime, m.buckets[(m.start+2)%uint32(len(m.buckets))].ledgerCloseTimestamp)
	require.Equal(t, 3, len(m.buckets))

	// append ledger 8 events, but all buckets are full, so we need to evict ledger 5
	require.NoError(t, m.append(8, ledger8CloseTime, ledger8Events))
	eventsAreEqual(t, ledger6Events, m.buckets[m.start].events)
	eventsAreEqual(t, ledger7Events, m.buckets[(m.start+1)%uint32(len(m.buckets))].events)
	eventsAreEqual(t, ledger8Events, m.buckets[(m.start+2)%uint32(len(m.buckets))].events)
	require.Equal(t, uint32(8), m.buckets[(m.start+2)%uint32(len(m.buckets))].ledgerSeq)
	require.Equal(t, ledger8CloseTime, m.buckets[(m.start+2)%uint32(len(m.buckets))].ledgerCloseTimestamp)
	require.Equal(t, 3, len(m.buckets))

	// append ledger 9 events, but all buckets are full, so we need to evict ledger 6
	require.NoError(t, m.append(9, ledger9CloseTime, ledger9Events))
	eventsAreEqual(t, ledger7Events, m.buckets[m.start].events)
	eventsAreEqual(t, ledger8Events, m.buckets[(m.start+1)%uint32(len(m.buckets))].events)
	eventsAreEqual(t, ledger9Events, m.buckets[(m.start+2)%uint32(len(m.buckets))].events)
	require.Equal(t, uint32(9), m.buckets[(m.start+2)%uint32(len(m.buckets))].ledgerSeq)
	require.Equal(t, ledger9CloseTime, m.buckets[(m.start+2)%uint32(len(m.buckets))].ledgerCloseTimestamp)
	require.Equal(t, 3, len(m.buckets))
}

func TestScanRangeValidation(t *testing.T) {
	m, err := NewMemoryStore(4)
	require.NoError(t, err)
	assertNoCalls := func(cursor Cursor, timestamp int64, contractEvent xdr.ContractEvent) bool {
		t.Fatalf("unexpected call")
		return true
	}
	err = m.Scan(Range{
		Start:      MinCursor,
		ClampStart: true,
		End:        MaxCursor,
		ClampEnd:   true,
	}, assertNoCalls)
	require.EqualError(t, err, "event store is empty")

	m = createStore(t)

	for _, testCase := range []struct {
		input Range
		err   string
	}{
		{
			Range{
				Start:      MinCursor,
				ClampStart: false,
				End:        MaxCursor,
				ClampEnd:   true,
			},
			"start is before oldest ledger",
		},
		{
			Range{
				Start:      Cursor{Ledger: 4},
				ClampStart: false,
				End:        MaxCursor,
				ClampEnd:   true,
			},
			"start is before oldest ledger",
		},
		{
			Range{
				Start:      MinCursor,
				ClampStart: true,
				End:        MaxCursor,
				ClampEnd:   false,
			},
			"end is after latest ledger",
		},
		{
			Range{
				Start:      Cursor{Ledger: 5},
				ClampStart: true,
				End:        Cursor{Ledger: 10},
				ClampEnd:   false,
			},
			"end is after latest ledger",
		},
		{
			Range{
				Start:      Cursor{Ledger: 10},
				ClampStart: true,
				End:        Cursor{Ledger: 3},
				ClampEnd:   true,
			},
			"start is after newest ledger",
		},
		{
			Range{
				Start:      Cursor{Ledger: 10},
				ClampStart: false,
				End:        Cursor{Ledger: 3},
				ClampEnd:   false,
			},
			"start is after newest ledger",
		},
		{
			Range{
				Start:      Cursor{Ledger: 9},
				ClampStart: false,
				End:        Cursor{Ledger: 10},
				ClampEnd:   true,
			},
			"start is after newest ledger",
		},
		{
			Range{
				Start:      Cursor{Ledger: 9},
				ClampStart: false,
				End:        Cursor{Ledger: 10},
				ClampEnd:   false,
			},
			"start is after newest ledger",
		},
		{
			Range{
				Start:      Cursor{Ledger: 2},
				ClampStart: true,
				End:        Cursor{Ledger: 3},
				ClampEnd:   false,
			},
			"start is not before end",
		},
		{
			Range{
				Start:      Cursor{Ledger: 2},
				ClampStart: false,
				End:        Cursor{Ledger: 3},
				ClampEnd:   false,
			},
			"start is before oldest ledger",
		},
		{
			Range{
				Start:      Cursor{Ledger: 6},
				ClampStart: false,
				End:        Cursor{Ledger: 6},
				ClampEnd:   false,
			},
			"start is not before end",
		},
	} {
		err := m.Scan(testCase.input, assertNoCalls)
		require.EqualError(t, err, testCase.err, testCase.input)
	}
}

func createStore(t *testing.T) *MemoryStore {
	m, err := NewMemoryStore(4)
	require.NoError(t, err)

	require.NoError(t, m.append(5, ledger5CloseTime, ledger5Events))
	require.NoError(t, m.append(6, ledger6CloseTime, nil))
	require.NoError(t, m.append(7, ledger7CloseTime, ledger7Events))
	require.NoError(t, m.append(8, ledger8CloseTime, ledger8Events))

	return m
}

func concat(slices ...[]event) []event {
	var result []event
	for _, slice := range slices {
		result = append(result, slice...)
	}
	return result
}

func TestScan(t *testing.T) {
	m := createStore(t)

	genEquivalentInputs := func(input Range) []Range {
		results := []Range{input}
		if !input.ClampStart {
			rangeCopy := input
			rangeCopy.ClampStart = true
			results = append(results, rangeCopy)
		}
		if !input.ClampEnd {
			rangeCopy := input
			rangeCopy.ClampEnd = true
			results = append(results, rangeCopy)
		}
		if !input.ClampStart && !input.ClampEnd {
			rangeCopy := input
			rangeCopy.ClampStart = true
			rangeCopy.ClampEnd = true
			results = append(results, rangeCopy)
		}
		return results
	}

	for _, testCase := range []struct {
		input    Range
		expected []event
	}{
		{
			Range{
				Start:      MinCursor,
				ClampStart: true,
				End:        MaxCursor,
				ClampEnd:   true,
			},
			concat(ledger5Events, ledger6Events, ledger7Events, ledger8Events),
		},
		{
			Range{
				Start:      Cursor{Ledger: 5},
				ClampStart: false,
				End:        Cursor{Ledger: 9},
				ClampEnd:   false,
			},
			concat(ledger5Events, ledger6Events, ledger7Events, ledger8Events),
		},
		{
			Range{
				Start:      Cursor{Ledger: 5, Tx: 1, Op: 2},
				ClampStart: false,
				End:        Cursor{Ledger: 9},
				ClampEnd:   false,
			},
			concat(ledger5Events[2:], ledger6Events, ledger7Events, ledger8Events),
		},
		{
			Range{
				Start:      Cursor{Ledger: 5, Tx: 3},
				ClampStart: false,
				End:        MaxCursor,
				ClampEnd:   true,
			},
			concat(ledger6Events, ledger7Events, ledger8Events),
		},
		{
			Range{
				Start:      Cursor{Ledger: 6},
				ClampStart: false,
				End:        MaxCursor,
				ClampEnd:   true,
			},
			concat(ledger7Events, ledger8Events),
		},
		{
			Range{
				Start:      Cursor{Ledger: 6, Tx: 1},
				ClampStart: false,
				End:        MaxCursor,
				ClampEnd:   true,
			},
			concat(ledger7Events, ledger8Events),
		},
		{
			Range{
				Start:      Cursor{Ledger: 8, Tx: 2, Op: 1, Event: 0},
				ClampStart: false,
				End:        MaxCursor,
				ClampEnd:   true,
			},
			ledger8Events[len(ledger8Events)-1:],
		},
		{
			Range{
				Start:      Cursor{Ledger: 8, Tx: 2, Op: 1, Event: 0},
				ClampStart: false,
				End:        Cursor{Ledger: 9},
				ClampEnd:   false,
			},
			ledger8Events[len(ledger8Events)-1:],
		},
		{
			Range{
				Start:      Cursor{Ledger: 5},
				ClampStart: false,
				End:        Cursor{Ledger: 7},
				ClampEnd:   false,
			},
			concat(ledger5Events, ledger6Events),
		},
		{
			Range{
				Start:      Cursor{Ledger: 5, Tx: 1, Op: 2},
				ClampStart: false,
				End:        Cursor{Ledger: 8, Tx: 1, Op: 4},
				ClampEnd:   false,
			},
			concat(ledger5Events[2:], ledger6Events, ledger7Events, ledger8Events[:1]),
		},
	} {
		for _, input := range genEquivalentInputs(testCase.input) {
			var events []event
			iterateAll := true
			f := func(cursor Cursor, ledgerCloseTimestamp int64, contractEvent xdr.ContractEvent) bool {
				require.Equal(t, ledgerCloseTime(cursor.Ledger), ledgerCloseTimestamp)
				events = append(events, event{
					contents:   contractEvent,
					txIndex:    cursor.Tx,
					opIndex:    cursor.Op,
					eventIndex: cursor.Event,
				})
				return iterateAll
			}
			require.NoError(t, m.Scan(input, f))
			eventsAreEqual(t, testCase.expected, events)
			if len(events) > 0 {
				events = nil
				iterateAll = false
				require.NoError(t, m.Scan(input, f))
				eventsAreEqual(t, []event{testCase.expected[0]}, events)
			}
		}
	}
}
