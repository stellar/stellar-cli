package db

import (
	"context"
	"fmt"
	"math/rand"
	"path"
	"sync"
	"testing"
	"time"

	"github.com/jmoiron/sqlx"
	"github.com/stretchr/testify/assert"

	"github.com/stellar/go/xdr"
)

func getLedgerEntryAndLatestLedgerSequenceWithErr(db *sqlx.DB, key xdr.LedgerKey) (bool, xdr.LedgerEntry, uint32, error) {
	tx, err := NewLedgerEntryReader(db).NewTx(context.Background())
	if err != nil {
		return false, xdr.LedgerEntry{}, 0, err
	}

	latestSeq, err := tx.GetLatestLedgerSequence()
	if err != nil {
		return false, xdr.LedgerEntry{}, 0, err
	}

	present, entry, err := tx.GetLedgerEntry(key)
	if err != nil {
		return false, xdr.LedgerEntry{}, 0, err
	}

	if err := tx.Done(); err != nil {
		return false, xdr.LedgerEntry{}, 0, err
	}
	return present, entry, latestSeq, nil
}

func getLedgerEntryAndLatestLedgerSequence(db *sqlx.DB, key xdr.LedgerKey) (bool, xdr.LedgerEntry, uint32) {
	present, entry, latestSeq, err := getLedgerEntryAndLatestLedgerSequenceWithErr(db, key)
	if err != nil {
		panic(err)
	}
	return present, entry, latestSeq
}

func TestGoldenPath(t *testing.T) {
	db := NewTestDB(t)
	// Check that we get an empty DB error
	_, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.Equal(t, ErrEmptyDB, err)

	tx, err := NewWriter(db).NewTx(context.Background(), 150)
	assert.NoError(t, err)
	writer := tx.LedgerEntryWriter()

	// Fill the DB with a single entry and fetch it
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
	assert.NoError(t, writer.UpsertLedgerEntry(key, entry))
	ledgerSequence := uint32(23)
	assert.NoError(t, tx.Commit(ledgerSequence))

	present, obtainedEntry, obtainedLedgerSequence := getLedgerEntryAndLatestLedgerSequence(db, key)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, obtainedEntry.Data.Type, xdr.LedgerEntryTypeContractData)
	assert.Equal(t, xdr.Hash{0xca, 0xfe}, obtainedEntry.Data.ContractData.ContractId)
	assert.Equal(t, six, *obtainedEntry.Data.ContractData.Val.U32)

	obtainedLedgerSequence, err = NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	// Do another round, overwriting the ledger entry
	tx, err = NewWriter(db).NewTx(context.Background(), 150)
	assert.NoError(t, err)
	writer = tx.LedgerEntryWriter()
	eight := xdr.Uint32(8)
	entry.Data.ContractData.Val.U32 = &eight

	assert.NoError(t, writer.UpsertLedgerEntry(key, entry))

	ledgerSequence = uint32(24)
	assert.NoError(t, tx.Commit(ledgerSequence))

	present, obtainedEntry, obtainedLedgerSequence = getLedgerEntryAndLatestLedgerSequence(db, key)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, eight, *obtainedEntry.Data.ContractData.Val.U32)

	// Do another round, deleting the ledger entry
	tx, err = NewWriter(db).NewTx(context.Background(), 150)
	assert.NoError(t, err)
	writer = tx.LedgerEntryWriter()
	assert.NoError(t, err)

	assert.NoError(t, writer.DeleteLedgerEntry(key))
	ledgerSequence = uint32(25)
	assert.NoError(t, tx.Commit(ledgerSequence))

	present, _, obtainedLedgerSequence = getLedgerEntryAndLatestLedgerSequence(db, key)
	assert.False(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	obtainedLedgerSequence, err = NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
}

func TestDeleteNonExistentLedgerEmpty(t *testing.T) {
	db := NewTestDB(t)

	// Simulate a ledger which creates and deletes a ledger entry
	// which would result in trying to delete a ledger entry which isn't there
	tx, err := NewWriter(db).NewTx(context.Background(), 150)
	assert.NoError(t, err)
	writer := tx.LedgerEntryWriter()

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
	key, _ := getContractDataLedgerEntry(data)
	assert.NoError(t, writer.DeleteLedgerEntry(key))
	ledgerSequence := uint32(23)
	assert.NoError(t, tx.Commit(ledgerSequence))

	// Make sure that the ledger number was submitted
	obtainedLedgerSequence, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	// And that the entry doesn't exist
	present, _, obtainedLedgerSequence := getLedgerEntryAndLatestLedgerSequence(db, key)
	assert.False(t, present)
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

// Make sure that (multiple, simultaneous) read transactions can happen while a write-transaction is ongoing,
// and write is only visible once the transaction is committed
func TestReadTxsDuringWriteTx(t *testing.T) {
	db := NewTestDB(t)

	// Check that we get an empty DB error
	_, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.Equal(t, ErrEmptyDB, err)

	// Start filling the DB with a single entry (enforce flushing right away)
	tx, err := NewWriter(db).NewTx(context.Background(), 0)
	assert.NoError(t, err)
	writer := tx.LedgerEntryWriter()

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
	assert.NoError(t, writer.UpsertLedgerEntry(key, entry))

	// Before committing the changes, make sure multiple concurrent transactions can query the DB
	readTx1, err := NewLedgerEntryReader(db).NewTx(context.Background())
	assert.NoError(t, err)
	readTx2, err := NewLedgerEntryReader(db).NewTx(context.Background())
	assert.NoError(t, err)

	_, err = readTx1.GetLatestLedgerSequence()
	assert.Equal(t, ErrEmptyDB, err)
	present, _, err := readTx1.GetLedgerEntry(key)
	assert.NoError(t, err)
	assert.False(t, present)
	assert.NoError(t, readTx1.Done())

	_, err = readTx2.GetLatestLedgerSequence()
	assert.Equal(t, ErrEmptyDB, err)
	present, _, err = readTx2.GetLedgerEntry(key)
	assert.NoError(t, err)
	assert.False(t, present)
	assert.NoError(t, readTx2.Done())

	// Finish the write transaction and check that the results are present
	ledgerSequence := uint32(23)
	assert.NoError(t, tx.Commit(ledgerSequence))

	obtainedLedgerSequence, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	present, obtainedEntry, obtainedLedgerSequence := getLedgerEntryAndLatestLedgerSequence(db, key)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, six, *obtainedEntry.Data.ContractData.Val.U32)
}

// Make sure that a write transaction can happen while multiple read transactions are ongoing,
// and write is only visible once the transaction is committed
func TestWriteTxsDuringReadTxs(t *testing.T) {
	db := NewTestDB(t)

	// Check that we get an empty DB error
	_, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.Equal(t, ErrEmptyDB, err)

	// Create a multiple read transactions, interleaved with the writing process

	// First read transaction, before the write transaction is created
	readTx1, err := NewLedgerEntryReader(db).NewTx(context.Background())
	assert.NoError(t, err)

	// Start filling the DB with a single entry (enforce flushing right away)
	tx, err := NewWriter(db).NewTx(context.Background(), 0)
	assert.NoError(t, err)
	writer := tx.LedgerEntryWriter()

	// Second read transaction, after the write transaction is created
	readTx2, err := NewLedgerEntryReader(db).NewTx(context.Background())
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
	assert.NoError(t, writer.UpsertLedgerEntry(key, entry))

	// Third read transaction, after the first insert has happened in the write transaction
	readTx3, err := NewLedgerEntryReader(db).NewTx(context.Background())
	assert.NoError(t, err)

	// Make sure that all the read transactions get an emptyDB error before and after the write transaction is committed
	for _, readTx := range []LedgerEntryReadTx{readTx1, readTx2, readTx3} {
		_, err = readTx.GetLatestLedgerSequence()
		assert.Equal(t, ErrEmptyDB, err)
		present, _, err := readTx.GetLedgerEntry(key)
		assert.NoError(t, err)
		assert.False(t, present)
	}

	// commit the write transaction
	ledgerSequence := uint32(23)
	assert.NoError(t, tx.Commit(ledgerSequence))

	for _, readTx := range []LedgerEntryReadTx{readTx1, readTx2, readTx3} {
		_, err = readTx.GetLatestLedgerSequence()
		assert.Equal(t, ErrEmptyDB, err)
		present, _, err := readTx.GetLedgerEntry(key)
		assert.NoError(t, err)
		assert.False(t, present)
	}

	// Check that the results are present in the transactions happening after the commit

	obtainedLedgerSequence, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	present, obtainedEntry, obtainedLedgerSequence := getLedgerEntryAndLatestLedgerSequence(db, key)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, six, *obtainedEntry.Data.ContractData.Val.U32)

	for _, readTx := range []LedgerEntryReadTx{readTx1, readTx2, readTx3} {
		assert.NoError(t, readTx.Done())
	}
}

// Check that we can have coexisting reader and writer goroutines without deadlocks or errors
func TestConcurrentReadersAndWriter(t *testing.T) {
	db := NewTestDB(t)

	contractID := xdr.Hash{0xca, 0xfe}
	done := make(chan struct{})
	var wg sync.WaitGroup
	writer := func() {
		defer wg.Done()
		val := xdr.Uint32(0)
		data := xdr.ContractDataEntry{
			ContractId: contractID,
			Key: xdr.ScVal{
				Type: xdr.ScValTypeScvU32,
				U32:  &val,
			},
			Val: xdr.ScVal{
				Type: xdr.ScValTypeScvU32,
				U32:  &val,
			},
		}
		for ledgerSequence := uint32(0); ledgerSequence < 1000; ledgerSequence++ {
			tx, err := NewWriter(db).NewTx(context.Background(), 10)
			assert.NoError(t, err)
			writer := tx.LedgerEntryWriter()
			for i := 0; i < 200; i++ {
				val++
				key, entry := getContractDataLedgerEntry(data)
				assert.NoError(t, writer.UpsertLedgerEntry(key, entry))
			}
			assert.NoError(t, tx.Commit(ledgerSequence))
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
			found, ledgerEntry, ledger, err := getLedgerEntryAndLatestLedgerSequenceWithErr(db, key)
			if err != nil {
				if err != ErrEmptyDB {
					t.Fatalf("reader %d failed with error %v\n", keyVal, err)
				}
			} else {
				// All entries should be found once the first write commit is done
				assert.True(t, found)
				fmt.Printf("reader %d: for ledger %d\n", keyVal, ledger)
				assert.Equal(t, xdr.Uint32(keyVal), *ledgerEntry.Data.ContractData.Val.U32)
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
	db := NewTestDB(b)
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
		tx, err := NewWriter(db).NewTx(context.Background(), 150)
		assert.NoError(b, err)
		writer := tx.LedgerEntryWriter()
		for j := 0; j < numEntriesPerOp; j++ {
			keyUint32 = xdr.Uint32(j)
			assert.NoError(b, writer.UpsertLedgerEntry(key, entry))
		}
		assert.NoError(b, tx.Commit(uint32(i+1)))
	}
	b.StopTimer()
}

func NewTestDB(tb testing.TB) *sqlx.DB {
	tmp := tb.TempDir()
	dbPath := path.Join(tmp, "db.sqlite")
	db, err := OpenSQLiteDB(dbPath)
	if err != nil {
		assert.NoError(tb, db.Close())
	}
	var ver []string
	assert.NoError(tb, db.Select(&ver, "SELECT sqlite_version()"))
	tb.Logf("using sqlite version: %v", ver)
	tb.Cleanup(func() {
		assert.NoError(tb, db.Close())
	})
	return db
}
