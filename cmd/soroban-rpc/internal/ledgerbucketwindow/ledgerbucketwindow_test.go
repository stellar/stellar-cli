package ledgerbucketwindow

import (
	"testing"

	"github.com/stretchr/testify/require"
)

var (
	ledger5CloseTime = ledgerCloseTime(5)
	ledger6CloseTime = ledgerCloseTime(6)
	ledger7CloseTime = ledgerCloseTime(7)
	ledger8CloseTime = ledgerCloseTime(8)
	ledger9CloseTime = ledgerCloseTime(9)
)

func ledgerCloseTime(seq uint32) int64 {
	return int64(seq)*25 + 100
}

func TestAppend(t *testing.T) {
	m, err := NewLedgerBucketWindow[int](3)
	require.NoError(t, err)

	// test appending first bucket of events
	evicted, err := m.Append(LedgerBucket[int]{
		LedgerSeq:            5,
		LedgerCloseTimestamp: ledger5CloseTime,
		BucketContent:        5,
	})
	require.NoError(t, err)
	require.Nil(t, evicted)
	require.Equal(t, uint32(5), m.buckets[m.start].LedgerSeq)
	require.Equal(t, ledger5CloseTime, m.buckets[m.start].LedgerCloseTimestamp)
	require.Equal(t, 5, m.buckets[m.start].BucketContent)
	require.Equal(t, 1, len(m.buckets))

	// the next bucket must follow the previous bucket (ledger 5)
	_, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            10,
		LedgerCloseTimestamp: 100,
		BucketContent:        10,
	})
	require.EqualError(
		t, err,
		"ledgers not contiguous: expected ledger sequence 6 but received 10",
	)
	_, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            4,
		LedgerCloseTimestamp: 100,
		BucketContent:        4,
	})
	require.EqualError(
		t, err,
		"ledgers not contiguous: expected ledger sequence 6 but received 4",
	)
	_, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            5,
		LedgerCloseTimestamp: 100,
		BucketContent:        5,
	})
	require.EqualError(
		t, err,
		"ledgers not contiguous: expected ledger sequence 6 but received 5",
	)
	// check that none of the calls above modified our buckets
	require.Equal(t, 5, m.buckets[m.start].BucketContent)
	require.Equal(t, 1, len(m.buckets))

	// append ledger 6 bucket, now we have two buckets filled
	evicted, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            6,
		LedgerCloseTimestamp: ledger6CloseTime,
		BucketContent:        6,
	})
	require.NoError(t, err)
	require.Nil(t, evicted)
	require.Equal(t, 5, m.buckets[m.start].BucketContent)
	require.Equal(t, 6, m.buckets[(m.start+1)%uint32(len(m.buckets))].BucketContent)
	require.Equal(t, uint32(6), m.buckets[(m.start+1)%uint32(len(m.buckets))].LedgerSeq)
	require.Equal(t, ledger6CloseTime, m.buckets[(m.start+1)%uint32(len(m.buckets))].LedgerCloseTimestamp)
	require.Equal(t, 2, len(m.buckets))

	// the next bucket of events must follow the previous bucket (ledger 6)
	_, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            10,
		LedgerCloseTimestamp: 100,
		BucketContent:        10,
	})
	require.EqualError(
		t, err,
		"ledgers not contiguous: expected ledger sequence 7 but received 10",
	)
	_, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            5,
		LedgerCloseTimestamp: 100,
		BucketContent:        5,
	})
	require.EqualError(
		t, err,
		"ledgers not contiguous: expected ledger sequence 7 but received 5",
	)
	_, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            6,
		LedgerCloseTimestamp: 100,
		BucketContent:        6,
	})
	require.EqualError(
		t, err,
		"ledgers not contiguous: expected ledger sequence 7 but received 6",
	)

	// append ledger 7 events, now we have all three buckets filled
	evicted, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            7,
		LedgerCloseTimestamp: ledger7CloseTime,
		BucketContent:        7,
	})
	require.NoError(t, err)
	require.Nil(t, evicted)
	require.Equal(t, 5, m.buckets[m.start].BucketContent)
	require.Equal(t, 6, m.buckets[(m.start+1)%uint32(len(m.buckets))].BucketContent)
	require.Equal(t, uint32(6), m.buckets[(m.start+1)%uint32(len(m.buckets))].LedgerSeq)
	require.Equal(t, ledger6CloseTime, m.buckets[(m.start+1)%uint32(len(m.buckets))].LedgerCloseTimestamp)
	require.Equal(t, 7, m.buckets[(m.start+2)%uint32(len(m.buckets))].BucketContent)
	require.Equal(t, uint32(7), m.buckets[(m.start+2)%uint32(len(m.buckets))].LedgerSeq)
	require.Equal(t, ledger7CloseTime, m.buckets[(m.start+2)%uint32(len(m.buckets))].LedgerCloseTimestamp)
	require.Equal(t, 3, len(m.buckets))

	// append ledger 8 events, but all buckets are full, so we need to evict ledger 5
	evicted, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            8,
		LedgerCloseTimestamp: ledger8CloseTime,
		BucketContent:        8,
	})
	require.NoError(t, err)
	require.Equal(t, 5, evicted.BucketContent)
	require.Equal(t, uint32(5), evicted.LedgerSeq)
	require.Equal(t, 6, m.buckets[m.start].BucketContent)
	require.Equal(t, 7, m.buckets[(m.start+1)%uint32(len(m.buckets))].BucketContent)
	require.Equal(t, uint32(7), m.buckets[(m.start+1)%uint32(len(m.buckets))].LedgerSeq)
	require.Equal(t, ledger7CloseTime, m.buckets[(m.start+1)%uint32(len(m.buckets))].LedgerCloseTimestamp)
	require.Equal(t, 8, m.buckets[(m.start+2)%uint32(len(m.buckets))].BucketContent)
	require.Equal(t, uint32(8), m.buckets[(m.start+2)%uint32(len(m.buckets))].LedgerSeq)
	require.Equal(t, ledger8CloseTime, m.buckets[(m.start+2)%uint32(len(m.buckets))].LedgerCloseTimestamp)
	require.Equal(t, 3, len(m.buckets))

	// append ledger 9 events, but all buckets are full, so we need to evict ledger 6
	evicted, err = m.Append(LedgerBucket[int]{
		LedgerSeq:            9,
		LedgerCloseTimestamp: ledger9CloseTime,
		BucketContent:        9,
	})
	require.NoError(t, err)
	require.Equal(t, 6, evicted.BucketContent)
	require.Equal(t, uint32(6), evicted.LedgerSeq)
	require.Equal(t, 7, m.buckets[m.start].BucketContent)
	require.Equal(t, 8, m.buckets[(m.start+1)%uint32(len(m.buckets))].BucketContent)
	require.Equal(t, uint32(8), m.buckets[(m.start+1)%uint32(len(m.buckets))].LedgerSeq)
	require.Equal(t, ledger8CloseTime, m.buckets[(m.start+1)%uint32(len(m.buckets))].LedgerCloseTimestamp)
	require.Equal(t, 9, m.buckets[(m.start+2)%uint32(len(m.buckets))].BucketContent)
	require.Equal(t, uint32(9), m.buckets[(m.start+2)%uint32(len(m.buckets))].LedgerSeq)
	require.Equal(t, ledger9CloseTime, m.buckets[(m.start+2)%uint32(len(m.buckets))].LedgerCloseTimestamp)
	require.Equal(t, 3, len(m.buckets))
}
