package db

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/stellar/go/xdr"
)

func createLedger(ledgerSequence uint32) xdr.LedgerCloseMeta {
	return xdr.LedgerCloseMeta{
		V: 2,
		V2: &xdr.LedgerCloseMetaV2{
			LedgerHeader: xdr.LedgerHeaderHistoryEntry{
				Hash: xdr.Hash{},
				Header: xdr.LedgerHeader{
					LedgerSeq: xdr.Uint32(ledgerSequence),
				},
			},
			TxSet: xdr.GeneralizedTransactionSet{
				V:       1,
				V1TxSet: &xdr.TransactionSetV1{},
			},
		},
	}
}

func assertLedgerRange(t *testing.T, reader LedgerReader, start, end uint32) {
	allLedgers, err := reader.GetAllLedgers(context.Background())
	assert.NoError(t, err)
	for i := start - 1; i <= end+1; i++ {
		ledger, exists, err := reader.GetLedger(context.Background(), i)
		assert.NoError(t, err)
		if i < start || i > end {
			assert.False(t, exists)
			continue
		}
		assert.True(t, exists)
		ledgerBinary, err := ledger.MarshalBinary()
		assert.NoError(t, err)
		expected := createLedger(i)
		expectedBinary, err := expected.MarshalBinary()
		assert.NoError(t, err)
		assert.Equal(t, expectedBinary, ledgerBinary)

		ledgerBinary, err = allLedgers[0].MarshalBinary()
		assert.NoError(t, err)
		assert.Equal(t, expectedBinary, ledgerBinary)
		allLedgers = allLedgers[1:]
	}
	assert.Empty(t, allLedgers)
}

func TestLedgers(t *testing.T) {
	db := NewTestDB(t)

	reader := NewLedgerReader(db)
	_, exists, err := reader.GetLedger(context.Background(), 1)
	assert.NoError(t, err)
	assert.False(t, exists)

	for i := 1; i <= 10; i++ {
		ledgerSequence := uint32(i)
		tx, err := NewWriter(db).NewTx(context.Background(), 150)
		assert.NoError(t, err)
		assert.NoError(t, tx.LedgerWriter().InsertLedger(createLedger(ledgerSequence)))
		assert.NoError(t, tx.Commit(ledgerSequence))
	}

	assertLedgerRange(t, reader, 1, 10)

	ledgerSequence := uint32(11)
	tx, err := NewWriter(db).NewTx(context.Background(), 150)
	assert.NoError(t, err)
	assert.NoError(t, tx.LedgerWriter().InsertLedger(createLedger(ledgerSequence)))
	assert.NoError(t, tx.LedgerWriter().TrimLedgers(ledgerSequence, 15))
	assert.NoError(t, tx.Commit(ledgerSequence))

	assertLedgerRange(t, reader, 1, 11)

	ledgerSequence = uint32(12)
	tx, err = NewWriter(db).NewTx(context.Background(), 150)
	assert.NoError(t, err)
	assert.NoError(t, tx.LedgerWriter().InsertLedger(createLedger(ledgerSequence)))
	assert.NoError(t, tx.LedgerWriter().TrimLedgers(ledgerSequence, 5))
	assert.NoError(t, tx.Commit(ledgerSequence))

	assertLedgerRange(t, reader, 8, 12)
}
