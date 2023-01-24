package ledgerentry_storage

import (
	"context"
	"database/sql"
	"database/sql/driver"
	"embed"
	"encoding/hex"
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
	metaTableName               = "metadata"
	latestLedgerSequenceMetaKey = "LatestLedgerSequence"
	ledgerCloseMetaTableName    = "ledger_close_meta"
)

type DB interface {
	LedgerEntryStorage
	GetLatestLedgerSequence() (uint32, error)
	GetLedger(sequence uint32) (xdr.LedgerCloseMeta, bool, error)
	GetAllLedgers() ([]xdr.LedgerCloseMeta, error)
	NewLedgerEntryUpdaterTx(forLedgerSequence uint32, maxBatchSize int) (LedgerEntryUpdaterTx, error)
}

type LedgerEntryUpdaterTx interface {
	TrimLedgers(retentionWindow uint32) error
	InsertLedger(ledger xdr.LedgerCloseMeta) error
	UpsertLedgerEntry(key xdr.LedgerKey, entry xdr.LedgerEntry) error
	DeleteLedgerEntry(key xdr.LedgerKey) error
	Done() error
}

type sqlDB struct {
	db                  *sqlx.DB
	postWriteCommitHook func() error
}

func OpenSQLiteDB(dbFilePath string) (DB, error) {
	// 1. Use Write-Ahead Logging (WAL).
	// 2. Disable WAL auto-checkpointing (we will do the checkpointing ourselves with wal_checkpoint pragmas
	//    after every write transaction).
	// 3. Use synchronous=NORMAL, which is faster and still safe in WAL mode.
	db, err := sqlx.Open("sqlite3", fmt.Sprintf("file:%s?_journal_mode=WAL&_wal_autocheckpoint=0&_synchronous=NORMAL", dbFilePath))
	if err != nil {
		return nil, errors.Wrap(err, "open failed")
	}

	postWriteCommitHook := func() error {
		_, err := db.Exec("PRAGMA wal_checkpoint(TRUNCATE)")
		return err
	}

	ret := &sqlDB{
		db:                  db,
		postWriteCommitHook: postWriteCommitHook,
	}

	if err = runMigrations(ret.db.DB, "sqlite3"); err != nil {
		_ = db.Close()
		return nil, errors.Wrap(err, "could not run migrations")
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
	if err = tx.Select(&results, sqlStr, args...); err != nil {
		return xdr.LedgerEntry{}, err
	}
	switch len(results) {
	case 0:
		return xdr.LedgerEntry{}, sql.ErrNoRows
	case 1:
		// expected length
	default:
		panic(fmt.Errorf("multiple entries (%d) for key %q in table %q", len(results), hex.EncodeToString([]byte(encodedKey)), ledgerEntriesTableName))
	}
	ledgerEntryBin := results[0]
	var result xdr.LedgerEntry
	if err = xdr.SafeUnmarshal([]byte(ledgerEntryBin), &result); err != nil {
		return xdr.LedgerEntry{}, err
	}
	return result, nil
}

func getLatestLedgerSequence(tx *sqlx.Tx) (uint32, error) {
	sqlStr, args, err := sq.Select("value").From(metaTableName).Where(sq.Eq{"key": latestLedgerSequenceMetaKey}).ToSql()
	if err != nil {
		return 0, err
	}
	var results []string
	if err = tx.Select(&results, sqlStr, args...); err != nil {
		return 0, err
	}
	switch len(results) {
	case 0:
		return 0, ErrEmptyDB
	case 1:
	// expected length on an initialized DB
	default:
		panic(fmt.Errorf("multiple entries (%d) for key %q in table %q", len(results), latestLedgerSequenceMetaKey, metaTableName))
	}
	latestLedgerStr := results[0]
	latestLedger, err := strconv.ParseUint(latestLedgerStr, 10, 32)
	if err != nil {
		return 0, err
	}
	return uint32(latestLedger), nil
}

func upsertLatestLedgerSequence(tx *sqlx.Tx, sequence uint32) error {
	sqlStr, args, err := sq.Replace(metaTableName).Values(latestLedgerSequenceMetaKey, fmt.Sprintf("%d", sequence)).ToSql()
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
	// Since it's a read-only transaction, we don't
	// care whether we commit it or roll it back as long as we close it
	defer tx.Rollback()
	ret, err := getLatestLedgerSequence(tx)
	if err != nil {
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
	// Since it's a read-only transaction, we don't
	// care whether we commit it or roll it back as long as we close it
	defer tx.Rollback()
	seq, err := getLatestLedgerSequence(tx)
	if err != nil {
		return xdr.LedgerEntry{}, false, 0, err
	}
	buffer := xdr.NewEncodingBuffer()
	entry, err := getLedgerEntry(tx, buffer, key)
	if err == sql.ErrNoRows {
		return xdr.LedgerEntry{}, false, seq, nil
	}
	if err != nil {
		return xdr.LedgerEntry{}, false, seq, err
	}
	return entry, true, seq, nil
}

// TODO : move these Scan() and Value() functions to xdr/db.go in the monorepo
type ledgerCloseMeta xdr.LedgerCloseMeta

// Scan reads from src into a ledgerCloseMeta  struct
func (t *ledgerCloseMeta) Scan(src any) error {
	return safeBase64Scan(src, t)
}

// Value implements the database/sql/driver Valuer interface.
func (c ledgerCloseMeta) Value() (driver.Value, error) {
	return xdr.MarshalBase64(c)
}

// safeBase64Scan scans from src (which should be either a []byte or string)
// into dest by using `SafeUnmarshalBase64`.
func safeBase64Scan(src, dest any) error {
	var val string
	switch src := src.(type) {
	case []byte:
		val = string(src)
	case string:
		val = src
	default:
		return fmt.Errorf("Invalid value for %T", dest)
	}

	return xdr.SafeUnmarshalBase64(val, dest)
}

// GetAllLedgers returns all ledgers in the database.
func (s *sqlDB) GetAllLedgers() ([]xdr.LedgerCloseMeta, error) {
	sqlStr, args, err := sq.Select("meta").From(ledgerCloseMetaTableName).OrderBy("sequence asc").ToSql()
	if err != nil {
		return nil, err
	}
	var results []ledgerCloseMeta
	err = s.db.Select(&results, sqlStr, args...)
	ledgers := make([]xdr.LedgerCloseMeta, len(results))
	for i, ledger := range results {
		ledgers[i] = xdr.LedgerCloseMeta(ledger)
	}
	return ledgers, err
}

// GetLedger fetches a ledger from the db.
func (s *sqlDB) GetLedger(sequence uint32) (xdr.LedgerCloseMeta, bool, error) {
	sqlStr, args, err := sq.Select("meta").From(ledgerCloseMetaTableName).Where(sq.Eq{"sequence": sequence}).ToSql()
	if err != nil {
		return xdr.LedgerCloseMeta{}, false, err
	}
	var results []ledgerCloseMeta
	if err = s.db.Select(&results, sqlStr, args...); err != nil {
		return xdr.LedgerCloseMeta{}, false, err
	}
	switch len(results) {
	case 0:
		return xdr.LedgerCloseMeta{}, false, nil
	case 1:
		return xdr.LedgerCloseMeta(results[0]), true, nil
	default:
		panic(fmt.Errorf("multiple lcm entries (%d) for sequence %d in table %q", len(results), sequence, ledgerCloseMetaTableName))
	}
}

func (s *sqlDB) Close() error {
	// TODO: What if there is a running transaction?
	return s.db.Close()
}

type ledgerUpdaterTx struct {
	tx                  *sqlx.Tx
	stmtCache           *sq.StmtCache
	postWriteCommitHook func() error
	// Value to set "latestSequence" to once we are done
	forLedgerSequence uint32
	maxBatchSize      int
	buffer            *xdr.EncodingBuffer
	// nil entries imply deletion
	keyToEntryBatch map[string]*string
}

func (s *sqlDB) NewLedgerEntryUpdaterTx(forLedgerSequence uint32, maxBatchSize int) (LedgerEntryUpdaterTx, error) {
	tx, err := s.db.BeginTxx(context.Background(), nil)
	if err != nil {
		return nil, err
	}
	ret := &ledgerUpdaterTx{
		tx:                  tx,
		stmtCache:           sq.NewStmtCache(tx),
		postWriteCommitHook: s.postWriteCommitHook,
		forLedgerSequence:   forLedgerSequence,
		maxBatchSize:        maxBatchSize,
		buffer:              xdr.NewEncodingBuffer(),
		keyToEntryBatch:     make(map[string]*string, maxBatchSize),
	}
	return ret, nil
}

func (l *ledgerUpdaterTx) flushLedgerEntryBatch() error {
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

// TrimLedgers removes all ledgers which are outside the retention window.
func (l *ledgerUpdaterTx) TrimLedgers(retentionWindow uint32) error {
	if l.forLedgerSequence+1 <= retentionWindow {
		return nil
	}
	cutoff := l.forLedgerSequence + 1 - retentionWindow
	deleteSQL := sq.StatementBuilder.RunWith(l.stmtCache).Delete(ledgerCloseMetaTableName).Where(sq.Lt{"sequence": cutoff})
	_, err := deleteSQL.Exec()
	return err
}

// InsertLedger inserts a ledger in the db.
func (l *ledgerUpdaterTx) InsertLedger(ledger xdr.LedgerCloseMeta) error {
	_, err := sq.StatementBuilder.RunWith(l.stmtCache).
		Insert(ledgerCloseMetaTableName).
		Values(l.forLedgerSequence, ledgerCloseMeta(ledger)).
		Exec()
	return err
}

func (l *ledgerUpdaterTx) UpsertLedgerEntry(key xdr.LedgerKey, entry xdr.LedgerEntry) error {
	if err := l.upsertLedgerEntry(key, entry); err != nil {
		_ = l.tx.Rollback()
		return err
	}
	return nil
}

// UpsertLedgerEntry() counterpart with no rollbacks (so that we only rollback in one place)
func (l *ledgerUpdaterTx) upsertLedgerEntry(key xdr.LedgerKey, entry xdr.LedgerEntry) error {
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
		if err := l.flushLedgerEntryBatch(); err != nil {
			return err
		}
		// reset map
		l.keyToEntryBatch = make(map[string]*string, maxBatchSize)
	}
	return nil
}

func (l *ledgerUpdaterTx) DeleteLedgerEntry(key xdr.LedgerKey) error {
	if err := l.deleteLedgerEntry(key); err != nil {
		_ = l.tx.Rollback()
		return err
	}
	return nil
}

// DeleteLedgerEntry() counterpart with no rollbacks (so that we only rollback in one place)
func (l *ledgerUpdaterTx) deleteLedgerEntry(key xdr.LedgerKey) error {
	encodedKey, err := encodeLedgerKey(l.buffer, key)
	if err != nil {
		return err
	}
	l.keyToEntryBatch[encodedKey] = nil
	if len(l.keyToEntryBatch) > l.maxBatchSize {
		if err := l.flushLedgerEntryBatch(); err != nil {
			return err
		}
		// reset map
		l.keyToEntryBatch = make(map[string]*string, maxBatchSize)
	}
	return nil
}

func (l *ledgerUpdaterTx) Done() error {
	if err := l.done(); err != nil {
		_ = l.tx.Rollback()
		return err
	}
	if err := l.tx.Commit(); err != nil {
		return err
	}
	if l.postWriteCommitHook != nil {
		if err := l.postWriteCommitHook(); err != nil {
			return err
		}
	}
	return nil
}

// Done() counterpart with no rollbacks or commits (so that we only rollback in one place)
func (l *ledgerUpdaterTx) done() error {
	if err := l.flushLedgerEntryBatch(); err != nil {
		return err
	}
	return upsertLatestLedgerSequence(l.tx, l.forLedgerSequence)
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
