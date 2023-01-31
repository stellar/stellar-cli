package db

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
	metaTableName                 = "metadata"
	latestLedgerSequenceMetaKey   = "LatestLedgerSequence"
	executeWALCheckpointFrequency = 1000
)

type ReadWriter interface {
	NewTx(ctx context.Context) (WriteTx, error)
	GetLatestLedgerSequence(ctx context.Context) (uint32, error)
}

type WriteTx interface {
	LedgerEntryWriter() LedgerEntryWriter
	LedgerWriter() LedgerWriter
	Commit(ledgerSeq uint32) error
	Rollback() error
}

func OpenSQLiteDB(dbFilePath string) (*sqlx.DB, error) {
	// 1. Use Write-Ahead Logging (WAL).
	// 2. Disable WAL auto-checkpointing (we will do the checkpointing ourselves with wal_checkpoint pragmas
	//    after every write transaction).
	// 3. Use synchronous=NORMAL, which is faster and still safe in WAL mode.
	db, err := sqlx.Open("sqlite3", fmt.Sprintf("file:%s?_journal_mode=WAL&_wal_autocheckpoint=0&_synchronous=NORMAL", dbFilePath))
	if err != nil {
		return nil, errors.Wrap(err, "open failed")
	}

	if err = runMigrations(db.DB, "sqlite3"); err != nil {
		_ = db.Close()
		return nil, errors.Wrap(err, "could not run migrations")
	}

	return db, nil
}

func getLatestLedgerSequence(ctx context.Context, q sqlx.QueryerContext) (uint32, error) {
	sqlStr, args, err := sq.Select("value").From(metaTableName).Where(sq.Eq{"key": latestLedgerSequenceMetaKey}).ToSql()
	if err != nil {
		return 0, err
	}
	var results []string
	if err = sqlx.SelectContext(ctx, q, &results, sqlStr, args...); err != nil {
		return 0, err
	}
	switch len(results) {
	case 0:
		return 0, ErrEmptyDB
	case 1:
		// expected length on an initialized DB
	default:
		return 0, fmt.Errorf("multiple entries (%d) for key %q in table %q", len(results), latestLedgerSequenceMetaKey, metaTableName)
	}
	latestLedgerStr := results[0]
	latestLedger, err := strconv.ParseUint(latestLedgerStr, 10, 32)
	if err != nil {
		return 0, err
	}
	return uint32(latestLedger), nil
}

type readWriter struct {
	txCounter             int
	db                    *sqlx.DB
	maxBatchSize          int
	ledgerRetentionWindow uint32
}

// NewReadWriter constructs a new ReadWriter instance and configures
// the size of ledger entry batches when writing ledger entries
// and the retention window for how many historical ledgers are
// recorded in the database.
func NewReadWriter(db *sqlx.DB, maxBatchSize int, ledgerRetentionWindow uint32) ReadWriter {
	return &readWriter{
		txCounter:             0,
		db:                    db,
		maxBatchSize:          maxBatchSize,
		ledgerRetentionWindow: ledgerRetentionWindow,
	}
}

func (rw *readWriter) GetLatestLedgerSequence(ctx context.Context) (uint32, error) {
	return getLatestLedgerSequence(ctx, rw.db)
}

func (rw *readWriter) NewTx(ctx context.Context) (WriteTx, error) {
	tx, err := rw.db.BeginTxx(ctx, nil)
	if err != nil {
		return nil, err
	}
	stmtCache := sq.NewStmtCache(tx)
	db := rw.db
	executeWALCheckpoint := rw.txCounter%executeWALCheckpointFrequency == 0
	rw.txCounter = (rw.txCounter + 1) % executeWALCheckpointFrequency
	return writeTx{
		postCommit: func() error {
			if !executeWALCheckpoint {
				return nil
			}
			_, err = db.ExecContext(ctx, "PRAGMA wal_checkpoint(TRUNCATE)")
			return err
		},
		tx:           tx,
		stmtCache:    stmtCache,
		ledgerWriter: ledgerWriter{stmtCache: stmtCache},
		ledgerEntryWriter: ledgerEntryWriter{
			buffer:          xdr.NewEncodingBuffer(),
			stmtCache:       stmtCache,
			keyToEntryBatch: make(map[string]*string, rw.maxBatchSize),
			maxBatchSize:    rw.maxBatchSize,
		},
		ledgerRetentionWindow: rw.ledgerRetentionWindow,
	}, nil
}

type writeTx struct {
	txCounter             int
	postCommit            func() error
	tx                    *sqlx.Tx
	stmtCache             *sq.StmtCache
	ledgerEntryWriter     ledgerEntryWriter
	ledgerWriter          ledgerWriter
	ledgerRetentionWindow uint32
}

func (w writeTx) LedgerEntryWriter() LedgerEntryWriter {
	return w.ledgerEntryWriter
}

func (w writeTx) LedgerWriter() LedgerWriter {
	return w.ledgerWriter
}

func (w writeTx) Commit(ledgerSeq uint32) error {
	if err := w.ledgerEntryWriter.flush(); err != nil {
		return err
	}

	if err := w.ledgerWriter.trimLedgers(ledgerSeq, w.ledgerRetentionWindow); err != nil {
		return err
	}

	_, err := sq.Replace(metaTableName).RunWith(w.stmtCache).
		Values(latestLedgerSequenceMetaKey, fmt.Sprintf("%d", ledgerSeq)).Exec()
	if err != nil {
		return err
	}

	if err = w.tx.Commit(); err != nil {
		return err
	}

	return w.postCommit()
}

func (w writeTx) Rollback() error {
	return w.tx.Rollback()
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
