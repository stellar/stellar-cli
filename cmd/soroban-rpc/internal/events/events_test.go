package events

import (
	"testing"

	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/require"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
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
)

func ledgerCloseTime(seq uint32) int64 {
	return int64(seq)*25 + 100
}

func newEvent(txIndex, opIndex, eventIndex, val uint32) event {
	v := xdr.Uint32(val)
	return event{
		contents: xdr.DiagnosticEvent{
			InSuccessfulContractCall: true,
			Event: xdr.ContractEvent{
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
		},
		txIndex:    txIndex,
		opIndex:    opIndex,
		eventIndex: eventIndex,
	}
}

func mustMarshal(e xdr.DiagnosticEvent) string {
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

func TestScanRangeValidation(t *testing.T) {
	m := NewMemoryStore(interfaces.MakeNoOpDeamon(), "unit-tests", 4)
	assertNoCalls := func(contractEvent xdr.DiagnosticEvent, cursor Cursor, timestamp int64) bool {
		t.Fatalf("unexpected call")
		return true
	}
	_, err := m.Scan(Range{
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
		_, err := m.Scan(testCase.input, assertNoCalls)
		require.EqualError(t, err, testCase.err, testCase.input)
	}
}

func createStore(t *testing.T) *MemoryStore {
	m := NewMemoryStore(interfaces.MakeNoOpDeamon(), "unit-tests", 4)
	m.eventsByLedger.Append(ledgerbucketwindow.LedgerBucket[[]event]{
		LedgerSeq:            5,
		LedgerCloseTimestamp: ledger5CloseTime,
		BucketContent:        ledger5Events,
	})
	m.eventsByLedger.Append(ledgerbucketwindow.LedgerBucket[[]event]{
		LedgerSeq:            6,
		LedgerCloseTimestamp: ledger6CloseTime,
		BucketContent:        nil,
	})
	m.eventsByLedger.Append(ledgerbucketwindow.LedgerBucket[[]event]{
		LedgerSeq:            7,
		LedgerCloseTimestamp: ledger7CloseTime,
		BucketContent:        ledger7Events,
	})
	m.eventsByLedger.Append(ledgerbucketwindow.LedgerBucket[[]event]{
		LedgerSeq:            8,
		LedgerCloseTimestamp: ledger8CloseTime,
		BucketContent:        ledger8Events,
	})

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
			f := func(contractEvent xdr.DiagnosticEvent, cursor Cursor, ledgerCloseTimestamp int64) bool {
				require.Equal(t, ledgerCloseTime(cursor.Ledger), ledgerCloseTimestamp)
				events = append(events, event{
					contents:   contractEvent,
					txIndex:    cursor.Tx,
					opIndex:    cursor.Op,
					eventIndex: cursor.Event,
				})
				return iterateAll
			}
			latest, err := m.Scan(input, f)
			require.NoError(t, err)
			require.Equal(t, uint32(8), latest)
			eventsAreEqual(t, testCase.expected, events)
			if len(events) > 0 {
				events = nil
				iterateAll = false
				latest, err := m.Scan(input, f)
				require.NoError(t, err)
				require.Equal(t, uint32(8), latest)
				eventsAreEqual(t, []event{testCase.expected[0]}, events)
			}
		}
	}
}
