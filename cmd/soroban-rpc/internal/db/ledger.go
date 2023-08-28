package db

import (
	"context"
	"fmt"

	sq "github.com/Masterminds/squirrel"

	"github.com/stellar/go/xdr"
)

const (
	ledgerCloseMetaTableName = "ledger_close_meta"
)

type StreamLedgerFn func(xdr.LedgerCloseMeta) error

type LedgerReader interface {
	GetLedger(ctx context.Context, sequence uint32) (xdr.LedgerCloseMeta, bool, error)
	StreamAllLedgers(ctx context.Context, f StreamLedgerFn) error
}

type LedgerWriter interface {
	InsertLedger(ledger xdr.LedgerCloseMeta) error
}

type ledgerReader struct {
	db *DB
}

func NewLedgerReader(db *DB) LedgerReader {
	return ledgerReader{db: db}
}

// StreamAllLedgers runs f over all the ledgers in the database (until f errors or signals it's done).
func (r ledgerReader) StreamAllLedgers(ctx context.Context, f StreamLedgerFn) error {
	sql := sq.Select("meta").From(ledgerCloseMetaTableName).OrderBy("sequence asc")
	q, err := r.db.Query(ctx, sql)
	if err != nil {
		return err
	}
	defer q.Close()
	for q.Next() {
		var closeMeta xdr.LedgerCloseMeta
		if err = q.Scan(&closeMeta); err != nil {
			return err
		}
		if err = f(closeMeta); err != nil {
			return err
		}
	}
	return nil
}

// GetLedger fetches a single ledger from the db.
func (r ledgerReader) GetLedger(ctx context.Context, sequence uint32) (xdr.LedgerCloseMeta, bool, error) {
	sql := sq.Select("meta").From(ledgerCloseMetaTableName).Where(sq.Eq{"sequence": sequence})
	var results []xdr.LedgerCloseMeta
	if err := r.db.Select(ctx, &results, sql); err != nil {
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
