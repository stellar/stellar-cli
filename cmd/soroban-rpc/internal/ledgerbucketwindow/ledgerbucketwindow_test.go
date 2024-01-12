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

	// Test appending first bucket of events
	evicted, err := m.Append(bucket(5))
	require.NoError(t, err)
	require.Nil(t, evicted)
	require.Equal(t, uint32(1), m.Len())

	firstBucket, err := m.Get(0)
	require.NoError(t, err)
	require.Equal(t, bucket(5), *firstBucket)

	// The next bucket must follow the previous bucket (ledger 5)
	_, err = m.Append(LedgerBucket[uint32]{
		LedgerSeq:            10,
		LedgerCloseTimestamp: 100,
		BucketContent:        10,
	})
	require.Errorf(t, err, "ledgers not contiguous: expected ledger sequence 6 but received 10")

	_, err = m.Append(LedgerBucket[uint32]{
		LedgerSeq:            4,
		LedgerCloseTimestamp: 100,
		BucketContent:        4,
	})
	require.Errorf(t, err, "ledgers not contiguous: expected ledger sequence 6 but received 4")

	// Check that none of the calls above modified our buckets
	require.Equal(t, uint32(1), m.Len())
	firstBucket, err = m.Get(0)
	require.NoError(t, err)
	require.Equal(t, bucket(5), *firstBucket)

	// Append ledger 6 bucket, now we have two buckets filled
	evicted, err = m.Append(bucket(6))
	require.NoError(t, err)
	require.Nil(t, evicted)
	require.Equal(t, uint32(2), m.Len())

	secondBucket, err := m.Get(1)
	require.NoError(t, err)
	require.Equal(t, bucket(6), *secondBucket)

	// Append ledger 7, now we have all three buckets filled
	evicted, err = m.Append(bucket(7))
	require.NoError(t, err)
	require.Nil(t, evicted)
	require.Equal(t, uint32(3), m.Len())

	thirdBucket, err := m.Get(2)
	require.NoError(t, err)
	require.Equal(t, bucket(7), *thirdBucket)

	// Append ledger 8, but all buckets are full, so we need to evict ledger 5
	evicted, err = m.Append(bucket(8))
	require.NoError(t, err)
	require.Equal(t, bucket(5), *evicted)
	require.Equal(t, uint32(3), m.Len())

	firstBucket, err = m.Get(0)
	require.NoError(t, err)
	require.Equal(t, bucket(6), *firstBucket)

	secondBucket, err = m.Get(1)
	require.NoError(t, err)
	require.Equal(t, bucket(7), *secondBucket)

	thirdBucket, err = m.Get(2)
	require.NoError(t, err)
	require.Equal(t, bucket(8), *thirdBucket)

	// Append ledger 9 events, but all buckets are full, so we need to evict ledger 6
	evicted, err = m.Append(bucket(9))
	require.NoError(t, err)
	require.Equal(t, bucket(6), *evicted)
	require.Equal(t, uint32(3), m.Len())

	firstBucket, err = m.Get(0)
	require.NoError(t, err)
	require.Equal(t, bucket(7), *firstBucket)

	secondBucket, err = m.Get(1)
	require.NoError(t, err)
	require.Equal(t, bucket(8), *secondBucket)

	thirdBucket, err = m.Get(2)
	require.NoError(t, err)
	require.Equal(t, bucket(9), *thirdBucket)

	// Append ledger 10, but all buckets are full, so we need to evict ledger 7.
	// The start index must have wrapped around
	evicted, err = m.Append(bucket(10))
	require.NoError(t, err)
	require.Equal(t, bucket(7), *evicted)
	require.Equal(t, uint32(3), m.Len())

	firstBucket, err = m.Get(0)
	require.NoError(t, err)
	require.Equal(t, bucket(8), *firstBucket)

	secondBucket, err = m.Get(1)
	require.NoError(t, err)
	require.Equal(t, bucket(9), *secondBucket)

	thirdBucket, err = m.Get(2)
	require.NoError(t, err)
	require.Equal(t, bucket(10), *thirdBucket)
}
