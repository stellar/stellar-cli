package ledgerentry_storage

import (
	"context"
	"database/sql"
	"fmt"
	"math/rand"
	"os"
	"path"
	"sync"
	"testing"
	"time"

	"github.com/jmoiron/sqlx"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"
)

func TestGoldenPath(t *testing.T) {
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
	assert.NoError(t, err)
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
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
}

func TestDeleteNonExistentLedgerEmpty(t *testing.T) {
	db, dbPath := NewTestDB()
	defer func() {
		assert.NoError(t, db.Close())
		assert.NoError(t, os.Remove(dbPath))
	}()

	// Simulate a ledger which creates and deletes a ledger entry
	// which would result in trying to delete a ledger entry which isn't there
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
	err = tx.DeleteLedgerEntry(key)
	assert.NoError(t, err)

	err = tx.Done()
	assert.NoError(t, err)

	// Make sure that the ledger number was submitted
	obtainedLedgerSequence, err := db.GetLatestLedgerSequence()
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	// And that the entry doesn't exist
	_, present, _, err := db.GetLedgerEntry(key)
	assert.NoError(t, err)
	assert.False(t, present)
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

// Make sure that (multiple, simultaneous) read transactions can happen while a write-transaction is ongoing,
// and write is only visible once the transaction is committed
func TestReadTxsDuringWriteTx(t *testing.T) {
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
	writeTx, err := db.NewLedgerEntryUpdaterTx(ledgerSequence, 0)

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
	err = writeTx.UpsertLedgerEntry(key, entry)
	assert.NoError(t, err)

	// Before committing the changes, make sure multiple concurrent transactions can query the DB
	internalDB := db.(*sqlDB).db
	readTx1, err := internalDB.BeginTxx(context.Background(), &sql.TxOptions{
		ReadOnly: true,
	})
	assert.NoError(t, err)
	readTx2, err := internalDB.BeginTxx(context.Background(), &sql.TxOptions{
		ReadOnly: true,
	})
	assert.NoError(t, err)
	_, err = getLatestLedgerSequence(readTx1)
	assert.Equal(t, ErrEmptyDB, err)
	_, err = getLedgerEntry(readTx1, xdr.NewEncodingBuffer(), key)
	assert.Equal(t, sql.ErrNoRows, err)
	assert.NoError(t, readTx1.Commit())

	_, err = getLatestLedgerSequence(readTx2)
	assert.Equal(t, ErrEmptyDB, err)
	_, err = getLedgerEntry(readTx2, xdr.NewEncodingBuffer(), key)
	assert.Equal(t, sql.ErrNoRows, err)
	assert.NoError(t, readTx2.Commit())

	// Finish the write transaction and check that the results are present
	assert.NoError(t, writeTx.Done())

	obtainedLedgerSequence, err := db.GetLatestLedgerSequence()
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	obtainedEntry, present, obtainedLedgerSequence, err := db.GetLedgerEntry(key)
	assert.NoError(t, err)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, six, *obtainedEntry.Data.ContractData.Val.U32)
}

// Make sure that a write transaction can happen while multiple read transactions are ongoing,
// and write is only visible once the transaction is committed
func TestWriteTxsDuringReadTxs(t *testing.T) {
	db, dbPath := NewTestDB()
	defer func() {
		assert.NoError(t, db.Close())
		assert.NoError(t, os.Remove(dbPath))
	}()

	// Check that we get an empty DB error
	_, err := db.GetLatestLedgerSequence()
	assert.Equal(t, ErrEmptyDB, err)

	// Create a multiple read transactions, interleaved with the writing process
	internalDB := db.(*sqlDB).db
	// First read transaction, before the write transaction is created
	readTx1, err := internalDB.BeginTxx(context.Background(), &sql.TxOptions{
		ReadOnly: true,
	})
	assert.NoError(t, err)

	// Start filling the DB with a single entry (enforce flushing right away)
	ledgerSequence := uint32(23)
	writeTx, err := db.NewLedgerEntryUpdaterTx(ledgerSequence, 0)

	// Second read transaction, after the write transaction is created
	readTx2, err := internalDB.BeginTxx(context.Background(), &sql.TxOptions{
		ReadOnly: true,
	})

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
	err = writeTx.UpsertLedgerEntry(key, entry)
	assert.NoError(t, err)

	// Third read transaction, after the first insert has happened in the write transaction
	readTx3, err := internalDB.BeginTxx(context.Background(), &sql.TxOptions{
		ReadOnly: true,
	})
	assert.NoError(t, err)

	// Make sure that all the read transactions get an emptyDB error before and after the write transaction is committed
	for _, readTx := range []*sqlx.Tx{readTx1, readTx2, readTx3} {
		_, err = getLatestLedgerSequence(readTx)
		assert.Equal(t, ErrEmptyDB, err)
		_, err = getLedgerEntry(readTx, xdr.NewEncodingBuffer(), key)
		assert.Equal(t, sql.ErrNoRows, err)
	}

	// commit the write transaction
	assert.NoError(t, writeTx.Done())

	for _, readTx := range []*sqlx.Tx{readTx1, readTx2, readTx3} {
		_, err = getLatestLedgerSequence(readTx)
		assert.Equal(t, ErrEmptyDB, err)
		_, err = getLedgerEntry(readTx, xdr.NewEncodingBuffer(), key)
		assert.Equal(t, sql.ErrNoRows, err)
	}

	// Check that the results are present in the transactions happening after the commit

	obtainedLedgerSequence, err := db.GetLatestLedgerSequence()
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	obtainedEntry, present, obtainedLedgerSequence, err := db.GetLedgerEntry(key)
	assert.NoError(t, err)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, six, *obtainedEntry.Data.ContractData.Val.U32)
}

// Check that we can have coexisting reader and writer goroutines without locks or errors
func TestConcurrentReadersAndWriter(t *testing.T) {
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
	contractID := xdr.Hash{0xca, 0xfe}
	done := make(chan struct{})
	var wg sync.WaitGroup
	writer := func() {
		defer wg.Done()
		zero := xdr.Uint32(0)
		data := xdr.ContractDataEntry{
			ContractId: contractID,
			Key: xdr.ScVal{
				Type: xdr.ScValTypeScvU32,
				U32:  &zero,
			},
			Val: xdr.ScVal{
				Type: xdr.ScValTypeScvU32,
				U32:  &zero,
			},
		}
		for ledgerSequence := uint32(0); ledgerSequence < 1000; ledgerSequence++ {
			writeTx, err := db.NewLedgerEntryUpdaterTx(ledgerSequence, 10)
			for i := 0; i < 200; i++ {
				*data.Key.U32 = (*data.Key.U32 + 1)
				*data.Val.U32 = (*data.Val.U32 + 1)
				key, entry := getContractDataLedgerEntry(data)
				err = writeTx.UpsertLedgerEntry(key, entry)
				assert.NoError(t, err)
			}
			err = writeTx.Done()
			assert.NoError(t, err)
			fmt.Printf("Wrote ledger %d\n", ledgerSequence)
			time.Sleep(time.Duration(rand.Int31n(30)) * time.Millisecond)
		}
		close(done)
	}
	reader := func(keyVal int) {
		defer wg.Done()
		val := xdr.Uint32(keyVal)
		key := xdr.LedgerKey{
			Type: xdr.LedgerEntryTypeContractData,
			ContractData: &xdr.LedgerKeyContractData{
				ContractId: contractID,
				Key: xdr.ScVal{
					Type: xdr.ScValTypeScvU32,
					U32:  &val,
				},
			},
		}
		for {
			select {
			case <-done:
				return
			default:
			}
			ledgerEntry, found, ledger, err := db.GetLedgerEntry(key)
			if err != nil {
				if err != ErrEmptyDB {
					t.Fatalf("reader %d failed with error %v\n", keyVal, err)
				}
			} else {
				fmt.Printf("reader %d: entry_present=%t for ledger %d\n", keyVal, found, ledger)
				if found {
					assert.Equal(t, xdr.Uint32(keyVal), *ledgerEntry.Data.ContractData.Val.U32)
				}
			}
			time.Sleep(time.Duration(rand.Int31n(30)) * time.Millisecond)
		}
	}

	// one writer, 32 readers
	wg.Add(1)
	go writer()

	for i := 1; i <= 32; i++ {
		wg.Add(1)
		go reader(i)
	}

	wg.Wait()
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
