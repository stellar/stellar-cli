package ledgerentry_storage

import (
	"context"
	"database/sql"
	"embed"
	"fmt"
	"strconv"

	sq "github.com/Masterminds/squirrel"
	"github.com/jmoiron/sqlx"
	_ "github.com/mattn/go-sqlite3"
	migrate "github.com/rubenv/sql-migrate"
	"github.com/stellar/go/support/errors"
	"github.com/stellar/go/xdr"
)

//go:embed migrations/*.sql
var migrations embed.FS

var ErrEmptyDB = errors.New("DB is empty")

const (
	ledgerEntriesTableName      = "ledger_entries"
	ledgerEntriesMetaTableName  = "ledger_entries_meta"
	latestLedgerSequenceMetaKey = "LatestLedgerSequence"
)

type DB interface {
	LedgerEntryStorage
	GetLatestLedgerSequence() (uint32, error)
	NewLedgerEntryUpdaterTx(forLedgerSequence uint32, maxBatchSize int) (LedgerEntryUpdaterTx, error)
}

type LedgerEntryUpdaterTx interface {
	UpsertLedgerEntry(key xdr.LedgerKey, entry xdr.LedgerEntry) error
	DeleteLedgerEntry(key xdr.LedgerKey) error
	Done() error
}

type sqlDB struct {
	db *sqlx.DB
}

func OpenSQLiteDB(dbFilePath string) (DB, error) {
	db, err := sqlx.Open("sqlite3", dbFilePath)
	if err != nil {
		return nil, errors.Wrap(err, "open failed")
	}

	ret := &sqlDB{
		db: db,
	}

	if err = runMigrations(ret.db.DB, "sqlite3"); err != nil {
		_ = db.Close()
	}

	return ret, nil
}

func getLedgerEntry(tx *sqlx.Tx, buffer *xdr.EncodingBuffer, key xdr.LedgerKey) (xdr.LedgerEntry, error) {
	encodedKey, err := encodeLedgerKey(buffer, key)
	if err != nil {
		return xdr.LedgerEntry{}, err
	}

	sqlStr, args, err := sq.Select("entry").From(ledgerEntriesTableName).Where(sq.Eq{"key": encodedKey}).ToSql()
	if err != nil {
		return xdr.LedgerEntry{}, err
	}
	var results []string
	if err := tx.Select(&results, sqlStr, args...); err != nil {
		return xdr.LedgerEntry{}, err
	}
	if len(results) != 1 {
		return xdr.LedgerEntry{}, sql.ErrNoRows
	}
	ledgerEntryBin := results[0]
	var result xdr.LedgerEntry
	if err = xdr.SafeUnmarshal([]byte(ledgerEntryBin), &result); err != nil {
		return xdr.LedgerEntry{}, err
	}
	return result, nil
}

func flushLedgerEntryBatch(tx *sqlx.Tx, encodedKeyEntries map[string]*string) error {
	upsertCount := 0
	upsertSQL := sq.Replace(ledgerEntriesTableName)
	var deleteKeys = make([]interface{}, 0, len(encodedKeyEntries))
	for key, entry := range encodedKeyEntries {
		if entry != nil {
			upsertSQL = upsertSQL.Values(key, entry)
			upsertCount += 1
		} else {
			deleteKeys = append(deleteKeys, interface{}(key))
		}
	}

	if upsertCount > 0 {
		sqlStr, args, err := upsertSQL.ToSql()
		if err != nil {
			return err
		}
		if _, err = tx.Exec(sqlStr, args...); err != nil {
			return err
		}
	}

	if len(deleteKeys) > 0 {
		sqlStr, args, err := sq.Delete(ledgerEntriesTableName).Where(sq.Eq{"key": deleteKeys}).ToSql()
		if err != nil {
			return err
		}
		_, err = tx.Exec(sqlStr, args...)
		if _, err = tx.Exec(sqlStr, args...); err != nil {
			return err
		}
	}
	return nil
}

func getLatestLedgerSequence(tx *sqlx.Tx) (uint32, error) {
	sqlStr, args, err := sq.Select("value").From(ledgerEntriesMetaTableName).Where(sq.Eq{"key": latestLedgerSequenceMetaKey}).ToSql()
	if err != nil {
		return 0, err
	}
	var results []string
	if err := tx.Select(&results, sqlStr, args...); err != nil {
		return 0, err
	}
	if len(results) != 1 {
		return 0, ErrEmptyDB
	}
	latestLedgerStr := results[0]
	latestLedger, err := strconv.ParseUint(latestLedgerStr, 10, 32)
	if err != nil {
		return 0, err
	}
	return uint32(latestLedger), nil
}

func upsertLatestLedgerSequence(tx *sqlx.Tx, sequence uint32) error {
	sqlStr, args, err := sq.Replace(ledgerEntriesMetaTableName).Values(latestLedgerSequenceMetaKey, fmt.Sprintf("%d", sequence)).ToSql()
	if err != nil {
		return err
	}
	_, err = tx.Exec(sqlStr, args...)
	return err
}

func (s *sqlDB) GetLatestLedgerSequence() (uint32, error) {
	opts := sql.TxOptions{
		ReadOnly: true,
	}
	tx, err := s.db.BeginTxx(context.Background(), &opts)
	if err != nil {
		return 0, err
	}
	ret, err := getLatestLedgerSequence(tx)
	if err != nil {
		_ = tx.Rollback()
		return 0, err
	}
	if err := tx.Commit(); err != nil {
		return 0, err
	}
	return ret, nil
}

func (s *sqlDB) GetLedgerEntry(key xdr.LedgerKey) (xdr.LedgerEntry, bool, uint32, error) {
	opts := sql.TxOptions{
		ReadOnly: true,
	}
	tx, err := s.db.BeginTxx(context.Background(), &opts)
	if err != nil {
		return xdr.LedgerEntry{}, false, 0, err
	}
	seq, err := getLatestLedgerSequence(tx)
	if err != nil {
		_ = tx.Rollback()
		return xdr.LedgerEntry{}, false, 0, err
	}
	buffer := xdr.NewEncodingBuffer()
	entry, err := getLedgerEntry(tx, buffer, key)
	if err == sql.ErrNoRows {
		return xdr.LedgerEntry{}, false, seq, nil
	}
	if err != nil {
		_ = tx.Rollback()
		return xdr.LedgerEntry{}, false, seq, err
	}
	if err := tx.Commit(); err != nil {
		return xdr.LedgerEntry{}, false, seq, err
	}
	return entry, true, seq, nil
}

func (s *sqlDB) Close() error {
	// TODO: What if there is a running transaction?
	return s.db.Close()
}

type ledgerUpdaterTx struct {
	tx *sqlx.Tx
	// Value to set "latestSequence" to once we are done
	forLedgerSequence uint32
	maxBatchSize      int
	buffer            *xdr.EncodingBuffer
	// nil implies deleted
	keyToEntryBatch map[string]*string
}

func (s *sqlDB) NewLedgerEntryUpdaterTx(forLedgerSequence uint32, maxBatchSize int) (LedgerEntryUpdaterTx, error) {
	tx, err := s.db.BeginTxx(context.Background(), nil)
	if err != nil {
		return nil, err
	}
	return &ledgerUpdaterTx{
		maxBatchSize:      maxBatchSize,
		tx:                tx,
		forLedgerSequence: forLedgerSequence,
		buffer:            xdr.NewEncodingBuffer(),
		keyToEntryBatch:   make(map[string]*string, maxBatchSize),
	}, nil
}

func (l *ledgerUpdaterTx) UpsertLedgerEntry(key xdr.LedgerKey, entry xdr.LedgerEntry) error {
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
	if len(l.keyToEntryBatch) >= l.maxBatchSize {
		if err := flushLedgerEntryBatch(l.tx, l.keyToEntryBatch); err != nil {
			_ = l.tx.Rollback()
			return err
		}
		// reset map
		l.keyToEntryBatch = make(map[string]*string, maxBatchSize)
	}
	return nil
}

func (l *ledgerUpdaterTx) DeleteLedgerEntry(key xdr.LedgerKey) error {
	encodedKey, err := encodeLedgerKey(l.buffer, key)
	if err != nil {
		return err
	}
	l.keyToEntryBatch[encodedKey] = nil
	if len(l.keyToEntryBatch) > l.maxBatchSize {
		if err := flushLedgerEntryBatch(l.tx, l.keyToEntryBatch); err != nil {
			_ = l.tx.Rollback()
			return err
		}
		// reset map
		l.keyToEntryBatch = make(map[string]*string, maxBatchSize)
	}
	return nil
}

func (l *ledgerUpdaterTx) Done() error {
	if err := flushLedgerEntryBatch(l.tx, l.keyToEntryBatch); err != nil {
		_ = l.tx.Rollback()
		return err
	}
	if err := upsertLatestLedgerSequence(l.tx, l.forLedgerSequence); err != nil {
		return err
	}
	return l.tx.Commit()
}

func encodeLedgerKey(buffer *xdr.EncodingBuffer, key xdr.LedgerKey) (string, error) {
	// this is safe since we are converting to string right away, which causes a copy
	binKey, err := buffer.LedgerKeyUnsafeMarshalBinaryCompress(key)
	if err != nil {
		return "", err
	}
	return string(binKey), nil
}

func runMigrations(db *sql.DB, dialect string) error {
	m := &migrate.AssetMigrationSource{
		Asset: migrations.ReadFile,
		AssetDir: func() func(string) ([]string, error) {
			return func(path string) ([]string, error) {
				dirEntry, err := migrations.ReadDir(path)
				if err != nil {
					return nil, err
				}
				entries := make([]string, 0)
				for _, e := range dirEntry {
					entries = append(entries, e.Name())
				}

				return entries, nil
			}
		}(),
		Dir: "migrations",
	}
	_, err := migrate.ExecMax(db, dialect, m, migrate.Up, 0)
	return err
}
