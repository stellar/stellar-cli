package db

import (
	"context"
	"database/sql"
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
	NewCachedTx(ctx context.Context) (LedgerEntryReadTx, error)
}

type LedgerKeyAndEntry struct {
	Key   xdr.LedgerKey
	Entry xdr.LedgerEntry
}

type LedgerEntryReadTx interface {
	GetLatestLedgerSequence() (uint32, error)
	GetLedgerEntries(keys ...xdr.LedgerKey) ([]LedgerKeyAndEntry, error)
	Done() error
}

type LedgerEntryWriter interface {
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
	globalCache            *dbCache
	stmtCache              *sq.StmtCache
	latestLedgerSeqCache   uint32
	ledgerEntryCacheReadTx *transactionalCacheReadTx
	tx                     db.SessionInterface
	buffer                 *xdr.EncodingBuffer
}

func (l *ledgerEntryReadTx) GetLatestLedgerSequence() (uint32, error) {
	if l.latestLedgerSeqCache != 0 {
		return l.latestLedgerSeqCache, nil
	}
	latestLedgerSeq, err := getLatestLedgerSequence(context.Background(), l.tx, l.globalCache)
	if err == nil {
		l.latestLedgerSeqCache = latestLedgerSeq
	}
	return latestLedgerSeq, err
}

// From compressed XDR keys to XDR entries (i.e. using the DB's representation)
func (l *ledgerEntryReadTx) getRawLedgerEntries(keys ...string) (map[string]string, error) {
	result := make(map[string]string, len(keys))
	keysToQueryInDB := keys
	if l.ledgerEntryCacheReadTx != nil {
		keysToQueryInDB = make([]string, 0, len(keys))
		for _, k := range keys {
			entry, ok := l.ledgerEntryCacheReadTx.get(k)
			if !ok {
				keysToQueryInDB = append(keysToQueryInDB, k)
			}
			if entry != nil {
				result[k] = *entry
			}
		}
	}

	if len(keysToQueryInDB) == 0 {
		return result, nil
	}

	builder := sq.StatementBuilder
	if l.stmtCache != nil {
		builder = builder.RunWith(l.stmtCache)
	} else {
		builder = builder.RunWith(l.tx.GetTx())
	}
	sql := builder.Select("key", "entry").From(ledgerEntriesTableName).Where(sq.Eq{"key": keysToQueryInDB})
	q, err := sql.Query()
	if err != nil {
		return nil, err
	}
	defer q.Close()
	for q.Next() {
		var key, entry string
		if err = q.Scan(&key, &entry); err != nil {
			return nil, err
		}
		result[key] = entry
		if l.ledgerEntryCacheReadTx != nil {
			l.ledgerEntryCacheReadTx.upsert(key, &entry)

			// Add missing config setting entries to the top cache.
			// Otherwise, the write-through cache won't get updated on restarts
			// (after which we don't process past config setting updates)
			keyType, err := xdr.GetBinaryCompressedLedgerKeyType([]byte(key))
			if err != nil {
				return nil, err
			}
			if keyType == xdr.LedgerEntryTypeConfigSetting {
				l.globalCache.Lock()
				// Only update the cache if the entry is missing, otherwise
				// we may end up overwriting the entry with an older version
				if _, ok := l.globalCache.ledgerEntries.entries[key]; !ok {
					l.globalCache.ledgerEntries.entries[key] = entry
				}
				defer l.globalCache.Unlock()
			}
		}
	}
	return result, nil
}

func GetLedgerEntry(tx LedgerEntryReadTx, key xdr.LedgerKey) (bool, xdr.LedgerEntry, error) {
	keyEntries, err := tx.GetLedgerEntries(key)
	if err != nil {
		return false, xdr.LedgerEntry{}, err
	}
	switch len(keyEntries) {
	case 0:
		return false, xdr.LedgerEntry{}, nil
	case 1:
		// expected length
		return true, keyEntries[0].Entry, nil
	default:
		return false, xdr.LedgerEntry{}, fmt.Errorf("multiple entries (%d) for key %v", len(keyEntries), key)
	}
}

func (l *ledgerEntryReadTx) GetLedgerEntries(keys ...xdr.LedgerKey) ([]LedgerKeyAndEntry, error) {
	encodedKeys := make([]string, len(keys))
	encodedKeyToKey := make(map[string]xdr.LedgerKey, len(keys))
	for i, k := range keys {
		encodedKey, err := encodeLedgerKey(l.buffer, k)
		if err != nil {
			return nil, err
		}
		encodedKeys[i] = encodedKey
		encodedKeyToKey[encodedKey] = k
	}

	rawResult, err := l.getRawLedgerEntries(encodedKeys...)
	if err != nil {
		return nil, err
	}

	result := make([]LedgerKeyAndEntry, 0, len(rawResult))
	for encodedKey, key := range encodedKeyToKey {
		encodedEntry, ok := rawResult[encodedKey]
		if !ok {
			continue
		}
		var entry xdr.LedgerEntry
		if err := xdr.SafeUnmarshal([]byte(encodedEntry), &entry); err != nil {
			return nil, errors.Wrap(err, "cannot decode ledger entry from DB")
		}
		result = append(result, LedgerKeyAndEntry{key, entry})
	}

	return result, nil
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
	return getLatestLedgerSequence(ctx, r.db, &r.db.cache)
}

// NewCachedTx() caches all accessed ledger entries and select statements. If many ledger entries are accessed, it will grow without bounds.
func (r ledgerEntryReader) NewCachedTx(ctx context.Context) (LedgerEntryReadTx, error) {
	txSession := r.db.Clone()
	// We need to copy the cached ledger entries locally when we start the transaction
	// since otherwise we would break the consistency between the transaction and the cache.

	// We need to make the parent cache access atomic with the read transaction creation.
	// Otherwise, the cache can be made inconsistent if a write transaction finishes
	// in between, updating the cache.
	r.db.cache.RLock()
	defer r.db.cache.RUnlock()
	if err := txSession.BeginTx(ctx, &sql.TxOptions{ReadOnly: true}); err != nil {
		return nil, err
	}
	cacheReadTx := r.db.cache.ledgerEntries.newReadTx()
	return &ledgerEntryReadTx{
		globalCache:            &r.db.cache,
		stmtCache:              sq.NewStmtCache(txSession.GetTx()),
		latestLedgerSeqCache:   r.db.cache.latestLedgerSeq,
		ledgerEntryCacheReadTx: &cacheReadTx,
		tx:                     txSession,
		buffer:                 xdr.NewEncodingBuffer(),
	}, nil
}

func (r ledgerEntryReader) NewTx(ctx context.Context) (LedgerEntryReadTx, error) {
	txSession := r.db.Clone()
	if err := txSession.BeginTx(ctx, &sql.TxOptions{ReadOnly: true}); err != nil {
		return nil, err
	}
	r.db.cache.RLock()
	defer r.db.cache.RUnlock()
	return &ledgerEntryReadTx{
		globalCache:          &r.db.cache,
		latestLedgerSeqCache: r.db.cache.latestLedgerSeq,
		tx:                   txSession,
		buffer:               xdr.NewEncodingBuffer(),
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
