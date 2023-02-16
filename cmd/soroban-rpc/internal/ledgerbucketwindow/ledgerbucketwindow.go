package ledgerbucketwindow

import (
	"errors"
	"fmt"
)

// LedgerBucketWindow is a sequence of buckets associated to a ledger window.
type LedgerBucketWindow[T any] struct {
	// buckets is a circular buffer where each cell represents
	// all events occurring within a specific ledger.
	buckets []LedgerBucket[T]
	// start is the index of the head in the circular buffer.
	start uint32
}

// LedgerBucket holds the content associated to a ledger
type LedgerBucket[T any] struct {
	LedgerSeq            uint32
	LedgerCloseTimestamp int64
	BucketContent        T
}

// NewLedgerBucketWindow creates a new LedgerBucketWindow
func NewLedgerBucketWindow[T any](retentionWindow uint32) (*LedgerBucketWindow[T], error) {
	if retentionWindow == 0 {
		return nil, errors.New("retention window must be positive")
	}
	return &LedgerBucketWindow[T]{
		buckets: make([]LedgerBucket[T], 0, retentionWindow),
	}, nil
}

// Append adds a new bucket to the window. If the window is full a bucket will be evicted and returned.
func (w *LedgerBucketWindow[T]) Append(sequence uint32, ledgerCloseTimestamp int64, bucketContent T) (*LedgerBucket[T], error) {
	length := uint32(len(w.buckets))
	if length > 0 {
		expectedLedgerSequence := w.buckets[w.start].LedgerSeq + length
		if expectedLedgerSequence != sequence {
			return nil, fmt.Errorf("ledgers not contiguous: expected ledger sequence %v but received %v", expectedLedgerSequence, sequence)
		}
	}

	nextBucket := LedgerBucket[T]{
		LedgerCloseTimestamp: ledgerCloseTimestamp,
		LedgerSeq:            sequence,
		BucketContent:        bucketContent,
	}
	var evicted *LedgerBucket[T]
	if length < uint32(cap(w.buckets)) {
		w.buckets = append(w.buckets, nextBucket)
	} else {
		index := (w.start + length) % uint32(len(w.buckets))
		saved := w.buckets[index]
		evicted = &saved
		w.buckets[index] = nextBucket
		w.start++
	}

	return evicted, nil
}

// Len returns the length (number of buckets in the window)
func (w *LedgerBucketWindow[T]) Len() uint32 {
	return uint32(len(w.buckets))
}

// Get obtains a bucket from the window
func (w *LedgerBucketWindow[T]) Get(i uint32) *LedgerBucket[T] {
	l := uint32(len(w.buckets))
	if i >= l {
		panic(fmt.Sprintf("index out of range [%d] with length %d", i, l))
	}
	index := (w.start + i) % l
	return &w.buckets[index]
}
