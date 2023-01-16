package ledgerentry_storage

import (
	"fmt"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"
	"math/rand"
	"os"
	"path"
	"testing"
)

func TestSimpleDB(t *testing.T) {
	db, dbPath := NewTestDB()
	defer func() {
		assert.NoError(t, db.Close())
		assert.NoError(t, os.Remove(dbPath))
	}()

	// Check that we get an empty DB error
	_, err := db.GetLatestLedgerSequence()
	assert.Equal(t, ErrEmptyDB, err)

	// Fill the DB with a single entry and fetch it
	ledgerSequence := uint32(23)
	tx, err := db.NewLedgerEntryUpdaterTx(ledgerSequence, 150)
	assert.NoError(t, err)

	four := xdr.Uint32(4)
	six := xdr.Uint32(6)
	data := xdr.ContractDataEntry{
		ContractId: xdr.Hash{0xca, 0xfe},
		Key: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &four,
		},
		Val: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &six,
		},
	}
	key, entry := getContractDataLedgerEntry(data)
	err = tx.UpsertLedgerEntry(key, entry)
	assert.NoError(t, err)
	err = tx.Done()
	assert.NoError(t, err)

	obtainedEntry, present, obtainedLedgerSequence, err := db.GetLedgerEntry(key)
	assert.NoError(t, err)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, obtainedEntry.Data.Type, xdr.LedgerEntryTypeContractData)
	assert.Equal(t, xdr.Hash{0xca, 0xfe}, obtainedEntry.Data.ContractData.ContractId)
	assert.Equal(t, six, *obtainedEntry.Data.ContractData.Val.U32)

	obtainedLedgerSequence, err = db.GetLatestLedgerSequence()
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	// Do another round, overwriting the ledger entry
	ledgerSequence = uint32(24)
	tx, err = db.NewLedgerEntryUpdaterTx(ledgerSequence, 150)
	assert.NoError(t, err)
	eight := xdr.Uint32(8)
	entry.Data.ContractData.Val.U32 = &eight

	err = tx.UpsertLedgerEntry(key, entry)
	assert.NoError(t, err)

	err = tx.Done()
	assert.NoError(t, err)

	obtainedEntry, present, obtainedLedgerSequence, err = db.GetLedgerEntry(key)
	assert.NoError(t, err)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, eight, *obtainedEntry.Data.ContractData.Val.U32)

	// Do another round, deleting the ledger entry
	ledgerSequence = uint32(25)
	tx, err = db.NewLedgerEntryUpdaterTx(ledgerSequence, 150)
	assert.NoError(t, err)

	err = tx.DeleteLedgerEntry(key)
	assert.NoError(t, err)
	err = tx.Done()
	assert.NoError(t, err)
	_, present, _, err = db.GetLedgerEntry(key)
	assert.NoError(t, err)
	assert.False(t, present)

	obtainedLedgerSequence, err = db.GetLatestLedgerSequence()
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
}

func getContractDataLedgerEntry(data xdr.ContractDataEntry) (xdr.LedgerKey, xdr.LedgerEntry) {
	entry := xdr.LedgerEntry{
		LastModifiedLedgerSeq: 1,
		Data: xdr.LedgerEntryData{
			Type:         xdr.LedgerEntryTypeContractData,
			ContractData: &data,
		},
		Ext: xdr.LedgerEntryExt{},
	}
	var key xdr.LedgerKey
	err := key.SetContractData(data.ContractId, data.Key)
	if err != nil {
		panic(err)
	}
	return key, entry
}

func TestConcurrency(t *testing.T) {
	// Make sure that reads can happen while a write-transaction is ongoing
	// and writes are only visible once the transaction is committed
	db, dbPath := NewTestDB()
	defer func() {
		assert.NoError(t, db.Close())
		assert.NoError(t, os.Remove(dbPath))
	}()

	// Check that we get an empty DB error
	_, err := db.GetLatestLedgerSequence()
	assert.Equal(t, ErrEmptyDB, err)

	// Start filling the DB with a single entry (enforce flushing right away)
	ledgerSequence := uint32(23)
	tx, err := db.NewLedgerEntryUpdaterTx(ledgerSequence, 0)

	assert.NoError(t, err)
	four := xdr.Uint32(4)
	six := xdr.Uint32(6)
	data := xdr.ContractDataEntry{
		ContractId: xdr.Hash{0xca, 0xfe},
		Key: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &four,
		},
		Val: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &six,
		},
	}
	key, entry := getContractDataLedgerEntry(data)
	err = tx.UpsertLedgerEntry(key, entry)
	assert.NoError(t, err)

	// Before committing the changes make sure we can query the DB
	_, err = db.GetLatestLedgerSequence()
	assert.Equal(t, ErrEmptyDB, err)
	_, _, _, err = db.GetLedgerEntry(key)
	assert.Equal(t, ErrEmptyDB, err)

	// Finish the transaction and check that the results are present
	err = tx.Done()
	assert.NoError(t, err)

	obtainedLedgerSequence, err := db.GetLatestLedgerSequence()
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	obtainedEntry, present, obtainedLedgerSequence, err := db.GetLedgerEntry(key)
	assert.NoError(t, err)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, six, *obtainedEntry.Data.ContractData.Val.U32)
}

func BenchmarkLedgerUpdate(b *testing.B) {
	db, dbPath := NewTestDB()
	defer func() {
		err := db.Close()
		if err != nil {
			panic(err)
		}
		err = os.Remove(dbPath)
		if err != nil {
			panic(err)
		}
	}()
	keyUint32 := xdr.Uint32(0)
	data := xdr.ContractDataEntry{
		ContractId: xdr.Hash{0xca, 0xfe},
		Key: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &keyUint32,
		},
		Val: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &keyUint32,
		},
	}
	key, entry := getContractDataLedgerEntry(data)
	const numEntriesPerOp = 3500
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		tx, err := db.NewLedgerEntryUpdaterTx(uint32(i+1), maxBatchSize)
		if err != nil {
			panic(err)
		}
		for j := 0; j < numEntriesPerOp; j++ {
			keyUint32 = xdr.Uint32(j)
			if err := tx.UpsertLedgerEntry(key, entry); err != nil {
				panic(err)
			}
		}
		if err := tx.Done(); err != nil {
			panic(err)
		}
	}
	b.StopTimer()
}

func NewTestDB() (DB, string) {
	dbPath := path.Join(os.TempDir(), fmt.Sprintf("%08x.sqlite", rand.Int63()))
	db, err := OpenSQLiteDB(dbPath)
	if err != nil {
		panic(err)
	}
	return db, dbPath
}
