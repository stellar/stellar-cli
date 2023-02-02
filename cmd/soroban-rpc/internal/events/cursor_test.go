package events

import (
	"math"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestParseCursor(t *testing.T) {
	for _, cursor := range []Cursor{
		{
			Ledger: math.MaxInt32,
			Tx:     1048575,
			Op:     4095,
			Event:  math.MaxInt32,
		},
		{
			Ledger: 0,
			Tx:     0,
			Op:     0,
			Event:  0,
		},
		{
			Ledger: 123,
			Tx:     10,
			Op:     5,
			Event:  1,
		},
	} {
		parsed, err := ParseCursor(cursor.String())
		assert.NoError(t, err)
		assert.Equal(t, cursor, parsed)
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
