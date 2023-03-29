package ledgerbucketwindow

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func bucket(ledgerSeq uint32) LedgerBucket[uint32] {
	return LedgerBucket[uint32]{
		LedgerSeq:            ledgerSeq,
		LedgerCloseTimestamp: int64(ledgerSeq)*25 + 100,
		BucketContent:        ledgerSeq,
	}
}

func TestAppend(t *testing.T) {
	m := NewLedgerBucketWindow[uint32](3)
	require.Equal(t, uint32(0), m.Len())

	// test appending first bucket of events
	evicted := m.Append(bucket(5))
	require.Nil(t, evicted)
	require.Equal(t, uint32(1), m.Len())
	require.Equal(t, bucket(5), *m.Get(0))

	// the next bucket must follow the previous bucket (ledger 5)

	require.PanicsWithError(
		t, "ledgers not contiguous: expected ledger sequence 6 but received 10",
		func() {
			m.Append(LedgerBucket[uint32]{
				LedgerSeq:            10,
				LedgerCloseTimestamp: 100,
				BucketContent:        10,
			})
		},
	)
	require.PanicsWithError(
		t, "ledgers not contiguous: expected ledger sequence 6 but received 4",
		func() {
			m.Append(LedgerBucket[uint32]{
				LedgerSeq:            4,
				LedgerCloseTimestamp: 100,
				BucketContent:        4,
			})
		},
	)
	require.PanicsWithError(
		t, "ledgers not contiguous: expected ledger sequence 6 but received 5",
		func() {
			m.Append(LedgerBucket[uint32]{
				LedgerSeq:            5,
				LedgerCloseTimestamp: 100,
				BucketContent:        5,
			})
		},
	)
	// check that none of the calls above modified our buckets
	require.Equal(t, uint32(1), m.Len())
	require.Equal(t, bucket(5), *m.Get(0))

	// append ledger 6 bucket, now we have two buckets filled
	evicted = m.Append(bucket(6))
	require.Nil(t, evicted)
	require.Equal(t, uint32(2), m.Len())
	require.Equal(t, bucket(5), *m.Get(0))
	require.Equal(t, bucket(6), *m.Get(1))

	// the next bucket of events must follow the previous bucket (ledger 6)
	require.PanicsWithError(
		t, "ledgers not contiguous: expected ledger sequence 7 but received 10",
		func() {
			m.Append(LedgerBucket[uint32]{
				LedgerSeq:            10,
				LedgerCloseTimestamp: 100,
				BucketContent:        10,
			})
		},
	)
	require.PanicsWithError(
		t, "ledgers not contiguous: expected ledger sequence 7 but received 4",
		func() {
			m.Append(LedgerBucket[uint32]{
				LedgerSeq:            4,
				LedgerCloseTimestamp: 100,
				BucketContent:        4,
			})
		},
	)
	require.PanicsWithError(
		t, "ledgers not contiguous: expected ledger sequence 7 but received 5",
		func() {
			m.Append(LedgerBucket[uint32]{
				LedgerSeq:            5,
				LedgerCloseTimestamp: 100,
				BucketContent:        5,
			})
		},
	)

	// append ledger 7, now we have all three buckets filled
	evicted = m.Append(bucket(7))
	require.Nil(t, evicted)
	require.Nil(t, evicted)
	require.Equal(t, uint32(3), m.Len())
	require.Equal(t, bucket(5), *m.Get(0))
	require.Equal(t, bucket(6), *m.Get(1))
	require.Equal(t, bucket(7), *m.Get(2))

	// append ledger 8, but all buckets are full, so we need to evict ledger 5
	evicted = m.Append(bucket(8))
	require.Equal(t, bucket(5), *evicted)
	require.Equal(t, uint32(3), m.Len())
	require.Equal(t, bucket(6), *m.Get(0))
	require.Equal(t, bucket(7), *m.Get(1))
	require.Equal(t, bucket(8), *m.Get(2))

	// append ledger 9 events, but all buckets are full, so we need to evict ledger 6
	evicted = m.Append(bucket(9))
	require.Equal(t, bucket(6), *evicted)
	require.Equal(t, uint32(3), m.Len())
	require.Equal(t, bucket(7), *m.Get(0))
	require.Equal(t, bucket(8), *m.Get(1))
	require.Equal(t, bucket(9), *m.Get(2))

	// append ledger 10, but all buckets are full, so we need to evict ledger 7.
	// The start index must have wrapped around
	evicted = m.Append(bucket(10))
	require.Equal(t, bucket(7), *evicted)
	require.Equal(t, uint32(3), m.Len())
	require.Equal(t, bucket(8), *m.Get(0))
	require.Equal(t, bucket(9), *m.Get(1))
	require.Equal(t, bucket(10), *m.Get(2))
}
