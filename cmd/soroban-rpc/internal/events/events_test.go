package events

import (
	"testing"

	"github.com/stretchr/testify/require"

	"github.com/stellar/go/xdr"
)

var (
	ledger5Events = []event{
		newEvent(1, 0, 0, 100),
		newEvent(1, 0, 1, 200),
		newEvent(2, 0, 0, 300),
		newEvent(2, 1, 0, 400),
	}
	ledger6Events []event = nil
	ledger7Events         = []event{
		newEvent(1, 0, 0, 500),
	}
	ledger8Events = []event{
		newEvent(1, 0, 0, 600),
		newEvent(2, 0, 0, 700),
		newEvent(2, 0, 1, 800),
		newEvent(2, 0, 2, 900),
		newEvent(2, 1, 0, 1000),
	}
)

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

func TestCursorCmp(t *testing.T) {
	for _, testCase := range []struct {
		a        Cursor
		b        Cursor
		expected int
	}{
		{MinCursor, MaxCursor, -1},
		{MinCursor, MinCursor, 0},
		{MaxCursor, MaxCursor, 0},
		{
			Cursor{Ledger: 1, Tx: 2, Op: 3, Event: 4},
			Cursor{Ledger: 1, Tx: 2, Op: 3, Event: 4},
			0,
		},
		{
			Cursor{Ledger: 5, Tx: 2, Op: 3, Event: 4},
			Cursor{Ledger: 7, Tx: 2, Op: 3, Event: 4},
			-1,
		},
		{
			Cursor{Ledger: 5, Tx: 2, Op: 3, Event: 4},
			Cursor{Ledger: 5, Tx: 7, Op: 3, Event: 4},
			-1,
		},
		{
			Cursor{Ledger: 5, Tx: 2, Op: 3, Event: 4},
			Cursor{Ledger: 5, Tx: 2, Op: 7, Event: 4},
			-1,
		},
		{
			Cursor{Ledger: 5, Tx: 2, Op: 3, Event: 4},
			Cursor{Ledger: 5, Tx: 2, Op: 3, Event: 7},
			-1,
		},
	} {
		a := testCase.a
		b := testCase.b
		expected := testCase.expected

		if got := a.Cmp(b); got != expected {
			t.Fatalf("expected (%v).Cmp(%v) to be %v but got %v", a, b, expected, got)
		}
		a, b = b, a
		expected *= -1
		if got := a.Cmp(b); got != expected {
			t.Fatalf("expected (%v).Cmp(%v) to be %v but got %v", a, b, expected, got)
		}
	}
}

func TestAppend(t *testing.T) {
	m, err := NewMemoryStore(3)
	require.NoError(t, err)

	require.NoError(t, m.append(5, ledger5Events))
	require.Equal(t, uint32(5), m.buckets[m.start].ledgerSeq)
	eventsAreEqual(t, ledger5Events, m.buckets[m.start].events)
	require.Equal(t, uint32(1), m.length)

	require.EqualError(
		t, m.append(10, ledger5Events),
		"events not contiguous: expected ledger sequence 6 but received 10",
	)
	require.EqualError(
		t, m.append(4, ledger5Events),
		"events not contiguous: expected ledger sequence 6 but received 4",
	)
	require.EqualError(
		t, m.append(5, nil),
		"events not contiguous: expected ledger sequence 6 but received 5",
	)
	require.Equal(t, ledger5Events, m.buckets[m.start].events)
	require.Equal(t, uint32(1), m.length)

	require.NoError(t, m.append(6, ledger6Events))
	eventsAreEqual(t, ledger5Events, m.buckets[m.start].events)
	eventsAreEqual(t, ledger6Events, m.buckets[(m.start+1)%uint32(len(m.buckets))].events)
	require.Equal(t, uint32(2), m.length)

	require.EqualError(
		t, m.append(10, ledger5Events),
		"events not contiguous: expected ledger sequence 7 but received 10",
	)
	require.EqualError(
		t, m.append(5, ledger5Events),
		"events not contiguous: expected ledger sequence 7 but received 5",
	)
	require.EqualError(
		t, m.append(6, nil),
		"events not contiguous: expected ledger sequence 7 but received 6",
	)

	require.NoError(t, m.append(7, ledger7Events))
	eventsAreEqual(t, ledger5Events, m.buckets[m.start].events)
	eventsAreEqual(t, ledger6Events, m.buckets[(m.start+1)%uint32(len(m.buckets))].events)
	eventsAreEqual(t, ledger7Events, m.buckets[(m.start+2)%uint32(len(m.buckets))].events)
	require.Equal(t, uint32(3), m.length)

	ledger8Events := []event{
		newEvent(1, 0, 0, 600),
	}
	require.NoError(t, m.append(8, ledger8Events))
	eventsAreEqual(t, ledger6Events, m.buckets[m.start].events)
	eventsAreEqual(t, ledger7Events, m.buckets[(m.start+1)%uint32(len(m.buckets))].events)
	eventsAreEqual(t, ledger8Events, m.buckets[(m.start+2)%uint32(len(m.buckets))].events)
	require.Equal(t, uint32(3), m.length)

	ledger9Events := []event{
		newEvent(1, 0, 0, 700),
	}
	require.NoError(t, m.append(9, ledger9Events))
	eventsAreEqual(t, ledger7Events, m.buckets[m.start].events)
	eventsAreEqual(t, ledger8Events, m.buckets[(m.start+1)%uint32(len(m.buckets))].events)
	eventsAreEqual(t, ledger9Events, m.buckets[(m.start+2)%uint32(len(m.buckets))].events)
	require.Equal(t, uint32(3), m.length)
}

func TestScanRangeValidation(t *testing.T) {
	m, err := NewMemoryStore(4)
	require.NoError(t, err)
	assertNoCalls := func(cursor Cursor, contractEvent xdr.ContractEvent) bool {
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
			"start is not before end",
		},
		{
			Range{
				Start:      Cursor{Ledger: 10},
				ClampStart: false,
				End:        Cursor{Ledger: 3},
				ClampEnd:   false,
			},
			"start is not before end",
		},
		{
			Range{
				Start:      Cursor{Ledger: 9},
				ClampStart: false,
				End:        Cursor{Ledger: 10},
				ClampEnd:   true,
			},
			"start is not before end",
		},
		{
			Range{
				Start:      Cursor{Ledger: 9},
				ClampStart: false,
				End:        Cursor{Ledger: 10},
				ClampEnd:   false,
			},
			"end is after latest ledger",
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

	require.NoError(t, m.append(5, ledger5Events))
	require.NoError(t, m.append(6, nil))
	require.NoError(t, m.append(7, ledger7Events))
	require.NoError(t, m.append(8, ledger8Events))

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
			f := func(cursor Cursor, contractEvent xdr.ContractEvent) bool {
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
