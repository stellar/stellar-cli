package db

import (
	"context"
	"fmt"

	sq "github.com/Masterminds/squirrel"
	"github.com/jmoiron/sqlx"

	"github.com/stellar/go/xdr"
)

const (
	ledgerCloseMetaTableName = "ledger_close_meta"
)

type LedgerReader interface {
	GetLedger(ctx context.Context, sequence uint32) (xdr.LedgerCloseMeta, bool, error)
	GetAllLedgers(ctx context.Context) ([]xdr.LedgerCloseMeta, error)
}

type LedgerWriter interface {
	InsertLedger(ledger xdr.LedgerCloseMeta) error
}

type ledgerReader struct {
	db *sqlx.DB
}

func NewLedgerReader(db *sqlx.DB) LedgerReader {
	return ledgerReader{db: db}
}

// GetAllLedgers returns all ledgers in the database.
func (r ledgerReader) GetAllLedgers(ctx context.Context) ([]xdr.LedgerCloseMeta, error) {
	sqlStr, args, err := sq.Select("meta").From(ledgerCloseMetaTableName).OrderBy("sequence asc").ToSql()
	if err != nil {
		return nil, err
	}
	var results []xdr.LedgerCloseMeta
	err = r.db.SelectContext(ctx, &results, sqlStr, args...)
	return results, err
}

// GetLedger fetches a single ledger from the db.
func (r ledgerReader) GetLedger(ctx context.Context, sequence uint32) (xdr.LedgerCloseMeta, bool, error) {
	sqlStr, args, err := sq.Select("meta").From(ledgerCloseMetaTableName).Where(sq.Eq{"sequence": sequence}).ToSql()
	if err != nil {
		return xdr.LedgerCloseMeta{}, false, err
	}
	var results []xdr.LedgerCloseMeta
	if err = r.db.SelectContext(ctx, &results, sqlStr, args...); err != nil {
		return xdr.LedgerCloseMeta{}, false, err
	}
	switch len(results) {
	case 0:
		return xdr.LedgerCloseMeta{}, false, nil
	case 1:
		return results[0], true, nil
	default:
		return xdr.LedgerCloseMeta{}, false, fmt.Errorf("multiple lcm entries (%d) for sequence %d in table %q", len(results), sequence, ledgerCloseMetaTableName)
	}
}

type ledgerWriter struct {
	stmtCache *sq.StmtCache
}

// trimLedgers removes all ledgers which fall outside the retention window.
func (l ledgerWriter) trimLedgers(latestLedgerSeq uint32, retentionWindow uint32) error {
	if latestLedgerSeq+1 <= retentionWindow {
		return nil
	}
	cutoff := latestLedgerSeq + 1 - retentionWindow
	deleteSQL := sq.StatementBuilder.RunWith(l.stmtCache).Delete(ledgerCloseMetaTableName).Where(sq.Lt{"sequence": cutoff})
	_, err := deleteSQL.Exec()
	return err
}

// InsertLedger inserts a ledger in the db.
func (l ledgerWriter) InsertLedger(ledger xdr.LedgerCloseMeta) error {
	_, err := sq.StatementBuilder.RunWith(l.stmtCache).
		Insert(ledgerCloseMetaTableName).
		Values(ledger.LedgerSequence(), ledger).
		Exec()
	return err
}
