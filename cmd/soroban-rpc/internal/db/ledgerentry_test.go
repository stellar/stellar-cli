package db

import (
	"context"
	"fmt"
	"math/rand"
	"path"
	"sync"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/stellar/go/xdr"
)

func getLedgerEntryAndLatestLedgerSequenceWithErr(db *DB, key xdr.LedgerKey) (bool, xdr.LedgerEntry, uint32, error) {
	tx, err := NewLedgerEntryReader(db).NewTx(context.Background())
	if err != nil {
		return false, xdr.LedgerEntry{}, 0, err
	}
	var doneErr error
	defer func() {
		doneErr = tx.Done()
	}()

	latestSeq, err := tx.GetLatestLedgerSequence()
	if err != nil {
		return false, xdr.LedgerEntry{}, 0, err
	}

	present, entry, err := GetLedgerEntry(tx, key)
	if err != nil {
		return false, xdr.LedgerEntry{}, 0, err
	}

	return present, entry, latestSeq, doneErr
}

func getLedgerEntryAndLatestLedgerSequence(t require.TestingT, db *DB, key xdr.LedgerKey) (bool, xdr.LedgerEntry, uint32) {
	present, entry, latestSeq, err := getLedgerEntryAndLatestLedgerSequenceWithErr(db, key)
	require.NoError(t, err)
	return present, entry, latestSeq
}

func TestGoldenPath(t *testing.T) {
	db := NewTestDB(t)
	// Check that we get an empty DB error
	_, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.Equal(t, ErrEmptyDB, err)

	tx, err := NewReadWriter(db, 150, 15).NewTx(context.Background())
	assert.NoError(t, err)
	writer := tx.LedgerEntryWriter()

	// Fill the DB with a single entry and fetch it
	four := xdr.Uint32(4)
	six := xdr.Uint32(6)
	data := xdr.ContractDataEntry{
		Contract: xdr.ScAddress{
			Type:       xdr.ScAddressTypeScAddressTypeContract,
			ContractId: &xdr.Hash{0xca, 0xfe},
		},
		Key: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &four,
		},
		Durability: xdr.ContractDataDurabilityPersistent,
		Val: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &six,
		},
	}
	key, entry := getContractDataLedgerEntry(t, data)
	assert.NoError(t, writer.UpsertLedgerEntry(entry))
	ledgerSequence := uint32(23)
	assert.NoError(t, tx.Commit(ledgerSequence))

	present, obtainedEntry, obtainedLedgerSequence := getLedgerEntryAndLatestLedgerSequence(t, db, key)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, obtainedEntry.Data.Type, xdr.LedgerEntryTypeContractData)
	assert.Equal(t, xdr.Hash{0xca, 0xfe}, *obtainedEntry.Data.ContractData.Contract.ContractId)
	assert.Equal(t, six, *obtainedEntry.Data.ContractData.Val.U32)

	obtainedLedgerSequence, err = NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	// Do another round, overwriting the ledger entry
	tx, err = NewReadWriter(db, 150, 15).NewTx(context.Background())
	assert.NoError(t, err)
	writer = tx.LedgerEntryWriter()
	eight := xdr.Uint32(8)
	entry.Data.ContractData.Val.U32 = &eight

	assert.NoError(t, writer.UpsertLedgerEntry(entry))

	ledgerSequence = uint32(24)
	assert.NoError(t, tx.Commit(ledgerSequence))

	present, obtainedEntry, obtainedLedgerSequence = getLedgerEntryAndLatestLedgerSequence(t, db, key)
	assert.True(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
	assert.Equal(t, eight, *obtainedEntry.Data.ContractData.Val.U32)

	// Do another round, deleting the ledger entry
	tx, err = NewReadWriter(db, 150, 15).NewTx(context.Background())
	assert.NoError(t, err)
	writer = tx.LedgerEntryWriter()
	assert.NoError(t, err)

	assert.NoError(t, writer.DeleteLedgerEntry(key))
	ledgerSequence = uint32(25)
	assert.NoError(t, tx.Commit(ledgerSequence))

	present, _, obtainedLedgerSequence = getLedgerEntryAndLatestLedgerSequence(t, db, key)
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
	tx, err := NewReadWriter(db, 150, 15).NewTx(context.Background())
	assert.NoError(t, err)
	writer := tx.LedgerEntryWriter()

	four := xdr.Uint32(4)
	six := xdr.Uint32(6)
	data := xdr.ContractDataEntry{
		Contract: xdr.ScAddress{
			Type:       xdr.ScAddressTypeScAddressTypeContract,
			ContractId: &xdr.Hash{0xca, 0xfe},
		},
		Key: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &four,
		},
		Durability: xdr.ContractDataDurabilityPersistent,
		Val: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &six,
		},
	}
	key, _ := getContractDataLedgerEntry(t, data)
	assert.NoError(t, writer.DeleteLedgerEntry(key))
	ledgerSequence := uint32(23)
	assert.NoError(t, tx.Commit(ledgerSequence))

	// Make sure that the ledger number was submitted
	obtainedLedgerSequence, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	// And that the entry doesn't exist
	present, _, obtainedLedgerSequence := getLedgerEntryAndLatestLedgerSequence(t, db, key)
	assert.False(t, present)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)
}

func getContractDataLedgerEntry(t require.TestingT, data xdr.ContractDataEntry) (xdr.LedgerKey, xdr.LedgerEntry) {
	entry := xdr.LedgerEntry{
		LastModifiedLedgerSeq: 1,
		Data: xdr.LedgerEntryData{
			Type:         xdr.LedgerEntryTypeContractData,
			ContractData: &data,
		},
		Ext: xdr.LedgerEntryExt{},
	}
	var key xdr.LedgerKey
	err := key.SetContractData(data.Contract, data.Key, data.Durability)
	require.NoError(t, err)
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
	tx, err := NewReadWriter(db, 0, 15).NewTx(context.Background())
	assert.NoError(t, err)
	writer := tx.LedgerEntryWriter()

	four := xdr.Uint32(4)
	six := xdr.Uint32(6)
	data := xdr.ContractDataEntry{
		Contract: xdr.ScAddress{
			Type:       xdr.ScAddressTypeScAddressTypeContract,
			ContractId: &xdr.Hash{0xca, 0xfe},
		},
		Key: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &four,
		},
		Val: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &six,
		},
	}
	key, entry := getContractDataLedgerEntry(t, data)
	assert.NoError(t, writer.UpsertLedgerEntry(entry))

	// Before committing the changes, make sure multiple concurrent transactions can query the DB
	readTx1, err := NewLedgerEntryReader(db).NewTx(context.Background())
	assert.NoError(t, err)
	readTx2, err := NewLedgerEntryReader(db).NewTx(context.Background())
	assert.NoError(t, err)

	_, err = readTx1.GetLatestLedgerSequence()
	assert.Equal(t, ErrEmptyDB, err)
	present, _, err := GetLedgerEntry(readTx1, key)
	assert.NoError(t, err)
	assert.False(t, present)
	assert.NoError(t, readTx1.Done())

	_, err = readTx2.GetLatestLedgerSequence()
	assert.Equal(t, ErrEmptyDB, err)
	present, _, err = GetLedgerEntry(readTx2, key)
	assert.NoError(t, err)
	assert.False(t, present)
	assert.NoError(t, readTx2.Done())

	// Finish the write transaction and check that the results are present
	ledgerSequence := uint32(23)
	assert.NoError(t, tx.Commit(ledgerSequence))

	obtainedLedgerSequence, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	present, obtainedEntry, obtainedLedgerSequence := getLedgerEntryAndLatestLedgerSequence(t, db, key)
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
	tx, err := NewReadWriter(db, 0, 15).NewTx(context.Background())
	assert.NoError(t, err)
	writer := tx.LedgerEntryWriter()

	// Second read transaction, after the write transaction is created
	readTx2, err := NewLedgerEntryReader(db).NewTx(context.Background())
	assert.NoError(t, err)

	four := xdr.Uint32(4)
	six := xdr.Uint32(6)
	data := xdr.ContractDataEntry{
		Contract: xdr.ScAddress{
			Type:       xdr.ScAddressTypeScAddressTypeContract,
			ContractId: &xdr.Hash{0xca, 0xfe},
		},
		Key: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &four,
		},
		Durability: xdr.ContractDataDurabilityPersistent,
		Val: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &six,
		},
	}
	key, entry := getContractDataLedgerEntry(t, data)
	assert.NoError(t, writer.UpsertLedgerEntry(entry))

	// Third read transaction, after the first insert has happened in the write transaction
	readTx3, err := NewLedgerEntryReader(db).NewTx(context.Background())
	assert.NoError(t, err)

	// Make sure that all the read transactions get an emptyDB error before and after the write transaction is committed
	for _, readTx := range []LedgerEntryReadTx{readTx1, readTx2, readTx3} {
		_, err = readTx.GetLatestLedgerSequence()
		assert.Equal(t, ErrEmptyDB, err)
		present, _, err := GetLedgerEntry(readTx, key)
		assert.NoError(t, err)
		assert.False(t, present)
	}

	// commit the write transaction
	ledgerSequence := uint32(23)
	assert.NoError(t, tx.Commit(ledgerSequence))

	for _, readTx := range []LedgerEntryReadTx{readTx1, readTx2, readTx3} {
		_, err = readTx.GetLatestLedgerSequence()
		assert.Equal(t, ErrEmptyDB, err)
		present, _, err := GetLedgerEntry(readTx, key)
		assert.NoError(t, err)
		assert.False(t, present)
	}

	// Check that the results are present in the transactions happening after the commit

	obtainedLedgerSequence, err := NewLedgerEntryReader(db).GetLatestLedgerSequence(context.Background())
	assert.NoError(t, err)
	assert.Equal(t, ledgerSequence, obtainedLedgerSequence)

	present, obtainedEntry, obtainedLedgerSequence := getLedgerEntryAndLatestLedgerSequence(t, db, key)
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
	logMessageCh := make(chan string, 1)
	writer := func() {
		defer wg.Done()
		data := func(i int) xdr.ContractDataEntry {
			val := xdr.Uint32(i)
			return xdr.ContractDataEntry{
				Contract: xdr.ScAddress{
					Type:       xdr.ScAddressTypeScAddressTypeContract,
					ContractId: &contractID,
				},
				Key: xdr.ScVal{
					Type: xdr.ScValTypeScvU32,
					U32:  &val,
				},
				Durability: xdr.ContractDataDurabilityPersistent,
				Val: xdr.ScVal{
					Type: xdr.ScValTypeScvU32,
					U32:  &val,
				},
			}
		}
		rw := NewReadWriter(db, 10, 15)
		for ledgerSequence := uint32(0); ledgerSequence < 1000; ledgerSequence++ {
			tx, err := rw.NewTx(context.Background())
			assert.NoError(t, err)
			writer := tx.LedgerEntryWriter()
			for i := 0; i < 200; i++ {
				_, entry := getContractDataLedgerEntry(t, data(i))
				assert.NoError(t, writer.UpsertLedgerEntry(entry))
			}
			assert.NoError(t, tx.Commit(ledgerSequence))
			logMessageCh <- fmt.Sprintf("Wrote ledger %d", ledgerSequence)
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
				Contract: xdr.ScAddress{
					Type:       xdr.ScAddressTypeScAddressTypeContract,
					ContractId: &contractID,
				},
				Key: xdr.ScVal{
					Type: xdr.ScValTypeScvU32,
					U32:  &val,
				},
				Durability: xdr.ContractDataDurabilityPersistent,
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
				logMessageCh <- fmt.Sprintf("reader %d: for ledger %d", keyVal, ledger)
				assert.Equal(t, xdr.Uint32(keyVal), *ledgerEntry.Data.ContractData.Val.U32)
			}
			time.Sleep(time.Duration(rand.Int31n(30)) * time.Millisecond)
		}
	}

	// one readWriter, 32 readers
	wg.Add(1)
	go writer()

	for i := 1; i <= 32; i++ {
		wg.Add(1)
		go reader(i)
	}

	workersExitCh := make(chan struct{})
	go func() {
		defer close(workersExitCh)
		wg.Wait()
	}()

forloop:
	for {
		select {
		case <-workersExitCh:
			break forloop
		case msg := <-logMessageCh:
			t.Log(msg)
		}
	}

}

func benchmarkLedgerEntry(b *testing.B, cached bool, includeExpired bool) {
	db := NewTestDB(b)
	keyUint32 := xdr.Uint32(0)
	data := xdr.ContractDataEntry{
		Contract: xdr.ScAddress{
			Type:       xdr.ScAddressTypeScAddressTypeContract,
			ContractId: &xdr.Hash{0xca, 0xfe},
		},
		Key: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &keyUint32,
		},
		Durability: xdr.ContractDataDurabilityPersistent,
		Val: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &keyUint32,
		},
	}
	key, entry := getContractDataLedgerEntry(b, data)
	tx, err := NewReadWriter(db, 150, 15).NewTx(context.Background())
	assert.NoError(b, err)
	assert.NoError(b, tx.LedgerEntryWriter().UpsertLedgerEntry(entry))
	assert.NoError(b, tx.Commit(2))
	reader := NewLedgerEntryReader(db)
	const numQueriesPerOp = 15
	b.ResetTimer()
	b.StopTimer()
	for i := 0; i < b.N; i++ {
		var readTx LedgerEntryReadTx
		var err error
		if cached {
			readTx, err = reader.NewCachedTx(context.Background())
		} else {
			readTx, err = reader.NewTx(context.Background())
		}
		assert.NoError(b, err)
		for i := 0; i < numQueriesPerOp; i++ {
			b.StartTimer()
			found, _, err := GetLedgerEntry(readTx, key)
			b.StopTimer()
			assert.NoError(b, err)
			assert.True(b, found)
		}
		assert.NoError(b, readTx.Done())
	}
}

func BenchmarkGetLedgerEntry(b *testing.B) {
	b.Run("With cache, include expired", func(b *testing.B) { benchmarkLedgerEntry(b, true, true) })
	b.Run("With cache, exclude expired", func(b *testing.B) { benchmarkLedgerEntry(b, true, false) })
	b.Run("Without cache, exclude expired", func(b *testing.B) { benchmarkLedgerEntry(b, false, false) })
}

func BenchmarkLedgerUpdate(b *testing.B) {
	db := NewTestDB(b)
	keyUint32 := xdr.Uint32(0)
	data := xdr.ContractDataEntry{
		Contract: xdr.ScAddress{
			Type:       xdr.ScAddressTypeScAddressTypeContract,
			ContractId: &xdr.Hash{0xca, 0xfe},
		},
		Key: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &keyUint32,
		},
		Durability: xdr.ContractDataDurabilityPersistent,
		Val: xdr.ScVal{
			Type: xdr.ScValTypeScvU32,
			U32:  &keyUint32,
		},
	}
	_, entry := getContractDataLedgerEntry(b, data)
	const numEntriesPerOp = 3500
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		tx, err := NewReadWriter(db, 150, 15).NewTx(context.Background())
		assert.NoError(b, err)
		writer := tx.LedgerEntryWriter()
		for j := 0; j < numEntriesPerOp; j++ {
			keyUint32 = xdr.Uint32(j)
			assert.NoError(b, writer.UpsertLedgerEntry(entry))
		}
		assert.NoError(b, tx.Commit(uint32(i+1)))
	}
}

func NewTestDB(tb testing.TB) *DB {
	tmp := tb.TempDir()
	dbPath := path.Join(tmp, "db.sqlite")
	db, err := OpenSQLiteDB(dbPath)
	if err != nil {
		assert.NoError(tb, db.Close())
	}
	tb.Cleanup(func() {
		assert.NoError(tb, db.Close())
	})
	return &DB{
		SessionInterface: db,
		cache: dbCache{
			ledgerEntries: newTransactionalCache(),
		},
	}
}
