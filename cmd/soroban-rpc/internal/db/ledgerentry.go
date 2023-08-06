package db

import (
	"context"
	"database/sql"
	"encoding/base64"
	"encoding/hex"
	"fmt"

	sq "github.com/Masterminds/squirrel"

	"github.com/stellar/go/support/db"
	"github.com/stellar/go/support/errors"
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
	GetLedgerEntry(key xdr.LedgerKey, includeExpired bool) (bool, xdr.LedgerEntry, error)
	Done() error
}

type LedgerEntryWriter interface {
	ExtendLedgerEntry(key xdr.LedgerKey, expirationLedgerSeq xdr.Uint32) error
	UpsertLedgerEntry(entry xdr.LedgerEntry) error
	DeleteLedgerEntry(key xdr.LedgerKey) error
}

type ledgerEntryWriter struct {
	stmtCache *sq.StmtCache
	buffer    *xdr.EncodingBuffer
	// nil entries imply deletion
	keyToEntryBatch         map[string]*xdr.LedgerEntry
	ledgerEntryCacheWriteTx transactionalCacheWriteTx
	maxBatchSize            int
}

func (l ledgerEntryWriter) ExtendLedgerEntry(key xdr.LedgerKey, expirationLedgerSeq xdr.Uint32) error {
	// TODO: How do we figure out the current expiration? We might need to read
	// from the DB, but in the case of creating a new entry and immediately
	// extending it, or extending multiple times in the same ledger, the
	// expirationLedgerSeq might be buffered but not flushed yet.
	if key.Type != xdr.LedgerEntryTypeContractCode && key.Type != xdr.LedgerEntryTypeContractData {
		panic("ExtendLedgerEntry can only be used for contract code and data")
	}

	encodedKey, err := encodeLedgerKey(l.buffer, key)
	if err != nil {
		return err
	}

	var entry xdr.LedgerEntry
	// See if we have a pending (unflushed) update for this key
	queued := l.keyToEntryBatch[encodedKey]
	if queued != nil {
		entry = *queued
	} else {
		var existing string
		// Nothing in the flush buffer. Load the entry from the db
		err = sq.StatementBuilder.RunWith(l.stmtCache).Select("entry").From(ledgerEntriesTableName).Where(sq.Eq{"key": encodedKey}).QueryRow().Scan(&existing)
		if err == sql.ErrNoRows {
			return fmt.Errorf("no entry for key %q in table %q", base64.StdEncoding.EncodeToString([]byte(encodedKey)), ledgerEntriesTableName)
		} else if err != nil {
			return err
		}
		// Unmarshal the existing entry
		if err := xdr.SafeUnmarshal([]byte(existing), &entry); err != nil {
			return err
		}
	}

	// Update the expiration
	switch entry.Data.Type {
	case xdr.LedgerEntryTypeContractData:
		entry.Data.ContractData.ExpirationLedgerSeq = expirationLedgerSeq
	case xdr.LedgerEntryTypeContractCode:
		entry.Data.ContractCode.ExpirationLedgerSeq = expirationLedgerSeq
	}

	// Marshal the entry back and stage it
	return l.UpsertLedgerEntry(entry)
}

func (l ledgerEntryWriter) UpsertLedgerEntry(entry xdr.LedgerEntry) error {
	// We can do a little extra validation to ensure the entry and key match,
	// because the key can be derived from the entry.
	key, err := entry.LedgerKey()
	if err != nil {
		return errors.Wrap(err, "could not get ledger key from entry")
	}

	encodedKey, err := encodeLedgerKey(l.buffer, key)
	if err != nil {
		return err
	}

	l.keyToEntryBatch[encodedKey] = &entry
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

	upsertCacheUpdates := make(map[string]*string, len(l.keyToEntryBatch))
	for key, entry := range l.keyToEntryBatch {
		if entry != nil {
			// safe since we cast to string right away
			encodedEntry, err := l.buffer.UnsafeMarshalBinary(entry)
			if err != nil {
				return err
			}
			encodedEntryStr := string(encodedEntry)
			upsertSQL = upsertSQL.Values(key, encodedEntryStr)
			upsertCount += 1
			// Only cache Config entries for now
			if entry.Data.Type == xdr.LedgerEntryTypeConfigSetting {
				upsertCacheUpdates[key] = &encodedEntryStr
			}
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
		for key, entry := range upsertCacheUpdates {
			l.ledgerEntryCacheWriteTx.upsert(key, *entry)
		}
	}

	if len(deleteKeys) > 0 {
		deleteSQL := sq.StatementBuilder.RunWith(l.stmtCache).Delete(ledgerEntriesTableName).Where(sq.Eq{"key": deleteKeys})
		if _, err := deleteSQL.Exec(); err != nil {
			return err
		}
		for _, key := range deleteKeys {
			l.ledgerEntryCacheWriteTx.delete(key)
		}
	}

	return nil
}

type ledgerEntryReadTx struct {
	cachedLatestLedgerSeq  uint32
	ledgerEntryCacheReadTx transactionalCacheReadTx
	tx                     db.SessionInterface
	buffer                 *xdr.EncodingBuffer
}

func (l *ledgerEntryReadTx) GetLatestLedgerSequence() (uint32, error) {
	if l.cachedLatestLedgerSeq != 0 {
		return l.cachedLatestLedgerSeq, nil
	}
	latestLedgerSeq, err := getLatestLedgerSequence(context.Background(), l.tx)
	if err != nil {
		l.cachedLatestLedgerSeq = latestLedgerSeq
	}
	return latestLedgerSeq, err
}

func (l *ledgerEntryReadTx) getBinaryLedgerEntry(key xdr.LedgerKey) (bool, string, error) {
	encodedKey, err := encodeLedgerKey(l.buffer, key)
	if err != nil {
		return false, "", err
	}

	entry, ok := l.ledgerEntryCacheReadTx.get(encodedKey)
	if ok {
		return ok, entry, nil
	}

	sql := sq.Select("entry").From(ledgerEntriesTableName).Where(sq.Eq{"key": encodedKey})
	var results []string
	if err = l.tx.Select(context.Background(), &results, sql); err != nil {
		return false, "", err
	}
	switch len(results) {
	case 0:
		return false, "", nil
	case 1:
		// expected length
	default:
		return false, "", fmt.Errorf("multiple entries (%d) for key %q in table %q", len(results), hex.EncodeToString([]byte(encodedKey)), ledgerEntriesTableName)
	}
	return true, results[0], nil
}

func (l *ledgerEntryReadTx) GetLedgerEntry(key xdr.LedgerKey, includeExpired bool) (bool, xdr.LedgerEntry, error) {
	found, ledgerEntryBin, err := l.getBinaryLedgerEntry(key)
	if err != nil || !found {
		return found, xdr.LedgerEntry{}, err
	}
	var result xdr.LedgerEntry
	if err := xdr.SafeUnmarshal([]byte(ledgerEntryBin), &result); err != nil {
		return false, xdr.LedgerEntry{}, err
	}

	// Disallow access to entries that have expired. Expiration excludes the
	// "current" ledger, which we are building.
	if !includeExpired {
		if expirationLedgerSeq, ok := result.Data.ExpirationLedgerSeq(); ok {
			latestClosedLedger, err := l.GetLatestLedgerSequence()
			if err != nil {
				return false, xdr.LedgerEntry{}, err
			}
			if expirationLedgerSeq <= xdr.Uint32(latestClosedLedger) {
				return false, xdr.LedgerEntry{}, nil
			}
		}
	}

	return true, result, nil
}

func (l ledgerEntryReadTx) Done() error {
	// Since it's a read-only transaction, we don't
	// care whether we commit it or roll it back as long as we close it
	return l.tx.Rollback()
}

type ledgerEntryReader struct {
	db *DB
}

func NewLedgerEntryReader(db *DB) LedgerEntryReader {
	return ledgerEntryReader{db: db}
}

func (r ledgerEntryReader) GetLatestLedgerSequence(ctx context.Context) (uint32, error) {
	return getLatestLedgerSequence(ctx, r.db)
}

func (r ledgerEntryReader) NewTx(ctx context.Context) (LedgerEntryReadTx, error) {
	txSession := r.db.Clone()
	// We need to copy the cached ledger entries locally when we start the transaction
	// since otherwise we would break the consistency between the transaction and the cache.

	// We need to make the cache snapshot atomic with the read transaction creation.
	// Otherwise, the cache can be made inconsistent if a write transaction finishes
	// in between, updating the cache.
	r.db.ledgerEntryCacheMutex.RLock()
	defer r.db.ledgerEntryCacheMutex.RUnlock()
	if err := txSession.BeginTx(ctx, &sql.TxOptions{ReadOnly: true}); err != nil {
		return nil, err
	}
	cacheReadTx := r.db.ledgerEntryCache.newReadTx()

	return &ledgerEntryReadTx{
		ledgerEntryCacheReadTx: cacheReadTx,
		tx:                     txSession,
		buffer:                 xdr.NewEncodingBuffer(),
	}, nil
}

func encodeLedgerKey(buffer *xdr.EncodingBuffer, key xdr.LedgerKey) (string, error) {
	// this is safe since we are converting to string right away, which causes a copy
	binKey, err := buffer.LedgerKeyUnsafeMarshalBinaryCompress(key)
	if err != nil {
		return "", err
	}
	return string(binKey), nil
}
