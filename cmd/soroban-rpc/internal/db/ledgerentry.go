package db

import (
	"context"
	"database/sql"
	"encoding/hex"
	"fmt"

	sq "github.com/Masterminds/squirrel"
	"github.com/jmoiron/sqlx"

	"github.com/stellar/go/xdr"
)

const (
	ledgerEntriesTableName = "ledger_entries"
)

type LedgerEntryReader interface {
	GetLatestLedgerSequence(ctx context.Context) (uint32, error)
	NewTx(ctx context.Context) (LedgerEntryReadTx, error)
}

type LedgerEntryReadTx interface {
	GetLatestLedgerSequence() (uint32, error)
	GetLedgerEntry(key xdr.LedgerKey) (bool, xdr.LedgerEntry, error)
	Done() error
}

type LedgerEntryWriter interface {
	UpsertLedgerEntry(key xdr.LedgerKey, entry xdr.LedgerEntry) error
	DeleteLedgerEntry(key xdr.LedgerKey) error
}

type ledgerEntryWriter struct {
	stmtCache *sq.StmtCache
	buffer    *xdr.EncodingBuffer
	// nil entries imply deletion
	keyToEntryBatch map[string]*string
	maxBatchSize    int
}

func (l ledgerEntryWriter) UpsertLedgerEntry(key xdr.LedgerKey, entry xdr.LedgerEntry) error {
	encodedKey, err := encodeLedgerKey(l.buffer, key)
	if err != nil {
		return err
	}
	// safe since we cast to string right away
	encodedEntry, err := l.buffer.UnsafeMarshalBinary(&entry)
	if err != nil {
		return err
	}
	encodedEntryStr := string(encodedEntry)
	l.keyToEntryBatch[encodedKey] = &encodedEntryStr
	return l.maybeFlush()
}

func (l ledgerEntryWriter) DeleteLedgerEntry(key xdr.LedgerKey) error {
	encodedKey, err := encodeLedgerKey(l.buffer, key)
	if err != nil {
		return err
	}
	l.keyToEntryBatch[encodedKey] = nil
	return l.maybeFlush()
}

func (l ledgerEntryWriter) maybeFlush() error {
	if len(l.keyToEntryBatch) >= l.maxBatchSize {
		return l.flush()
	}
	return nil
}

func (l ledgerEntryWriter) flush() error {
	upsertCount := 0
	upsertSQL := sq.StatementBuilder.RunWith(l.stmtCache).Replace(ledgerEntriesTableName)
	var deleteKeys = make([]string, 0, len(l.keyToEntryBatch))

	for key, entry := range l.keyToEntryBatch {
		if entry != nil {
			upsertSQL = upsertSQL.Values(key, entry)
			upsertCount += 1
		} else {
			deleteKeys = append(deleteKeys, key)
		}
		// Delete each entry instead of reassigning l.keyToEntryBatch
		// to the empty map because the map was allocated with a
		// capacity of: make(map[string]*string, rw.maxBatchSize).
		// We want to reuse the hashtable buckets in subsequent
		// calls to UpsertLedgerEntry / DeleteLedgerEntry.
		delete(l.keyToEntryBatch, key)
	}

	if upsertCount > 0 {
		if _, err := upsertSQL.Exec(); err != nil {
			return err
		}
	}

	if len(deleteKeys) > 0 {
		deleteSQL := sq.StatementBuilder.RunWith(l.stmtCache).Delete(ledgerEntriesTableName).Where(sq.Eq{"key": deleteKeys})
		if _, err := deleteSQL.Exec(); err != nil {
			return err
		}
	}

	return nil
}

type ledgerEntryReadTx struct {
	tx     *sqlx.Tx
	buffer *xdr.EncodingBuffer
}

func (l ledgerEntryReadTx) GetLatestLedgerSequence() (uint32, error) {
	return getLatestLedgerSequence(context.Background(), l.tx)
}

func (l ledgerEntryReadTx) GetLedgerEntry(key xdr.LedgerKey) (bool, xdr.LedgerEntry, error) {
	encodedKey, err := encodeLedgerKey(l.buffer, key)
	if err != nil {
		return false, xdr.LedgerEntry{}, err
	}

	sqlStr, args, err := sq.Select("entry").From(ledgerEntriesTableName).Where(sq.Eq{"key": encodedKey}).ToSql()
	if err != nil {
		return false, xdr.LedgerEntry{}, err
	}
	var results []string
	if err = l.tx.Select(&results, sqlStr, args...); err != nil {
		return false, xdr.LedgerEntry{}, err
	}
	switch len(results) {
	case 0:
		return false, xdr.LedgerEntry{}, nil
	case 1:
		// expected length
	default:
		return false, xdr.LedgerEntry{}, fmt.Errorf("multiple entries (%d) for key %q in table %q", len(results), hex.EncodeToString([]byte(encodedKey)), ledgerEntriesTableName)
	}
	ledgerEntryBin := results[0]
	var result xdr.LedgerEntry
	if err = xdr.SafeUnmarshal([]byte(ledgerEntryBin), &result); err != nil {
		return false, xdr.LedgerEntry{}, err
	}
	return true, result, nil
}

func (l ledgerEntryReadTx) Done() error {
	// Since it's a read-only transaction, we don't
	// care whether we commit it or roll it back as long as we close it
	return l.tx.Rollback()
}

type ledgerEntryReader struct {
	db *sqlx.DB
}

func NewLedgerEntryReader(db *sqlx.DB) LedgerEntryReader {
	return ledgerEntryReader{db: db}
}

func (r ledgerEntryReader) GetLatestLedgerSequence(ctx context.Context) (uint32, error) {
	return getLatestLedgerSequence(ctx, r.db)
}

func (r ledgerEntryReader) NewTx(ctx context.Context) (LedgerEntryReadTx, error) {
	tx, err := r.db.BeginTxx(ctx, &sql.TxOptions{
		ReadOnly: true,
	})
	return ledgerEntryReadTx{
		tx:     tx,
		buffer: xdr.NewEncodingBuffer(),
	}, err
}

func encodeLedgerKey(buffer *xdr.EncodingBuffer, key xdr.LedgerKey) (string, error) {
	// this is safe since we are converting to string right away, which causes a copy
	binKey, err := buffer.LedgerKeyUnsafeMarshalBinaryCompress(key)
	if err != nil {
		return "", err
	}
	return string(binKey), nil
}
