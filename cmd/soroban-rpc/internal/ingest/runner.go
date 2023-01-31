package ingest

import (
	"context"
	"errors"
	"sync"
	"time"

	"github.com/stellar/go/historyarchive"
	"github.com/stellar/go/ingest"
	backends "github.com/stellar/go/ingest/ledgerbackend"
	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

const (
	changePrintOutFreq = 10000
)

type Config struct {
	Logger            *log.Entry
	DB                db.Writer
	NetworkPassPhrase string
	Archive           historyarchive.ArchiveInterface
	LedgerBackend     backends.LedgerBackend
	Timeout           time.Duration
}

func NewRunner(cfg Config) (*Runner, error) {
	ctx, done := context.WithCancel(context.Background())
	o := Runner{
		logger:            cfg.Logger,
		db:                cfg.DB,
		ledgerBackend:     cfg.LedgerBackend,
		networkPassPhrase: cfg.NetworkPassPhrase,
		timeout:           cfg.Timeout,
		done:              done,
	}
	o.wg.Add(1)
	go func() {
		err := o.run(ctx, cfg.Archive)
		if err != nil && !errors.Is(err, context.Canceled) {
			o.logger.WithError(err).Fatal("could not run ingestion")
		}
	}()
	return &o, nil
}

type Runner struct {
	logger                *log.Entry
	db                    db.Writer
	ledgerBackend         backends.LedgerBackend
	timeout               time.Duration
	ledgerRetentionWindow uint32
	networkPassPhrase     string
	done                  context.CancelFunc
	wg                    sync.WaitGroup
}

func (r *Runner) Close() error {
	r.done()
	r.wg.Wait()
	return nil
}

func (r *Runner) run(ctx context.Context, archive historyarchive.ArchiveInterface) error {
	defer r.wg.Done()
	nextLedgerSeq, checkPointFillErr, err := r.maybeFillEntriesFromCheckpoint(ctx, archive)
	if err != nil {
		return err
	}

	prepareRangeCtx, cancelPrepareRange := context.WithTimeout(ctx, r.timeout)
	if err := r.ledgerBackend.PrepareRange(prepareRangeCtx, backends.UnboundedRange(nextLedgerSeq)); err != nil {
		cancelPrepareRange()
		return err
	}
	cancelPrepareRange()

	// Make sure that the checkpoint prefill (if any), happened before starting to apply deltas
	if err := <-checkPointFillErr; err != nil {
		return err
	}

	for ; ctx.Err() == nil; nextLedgerSeq++ {
		if err := r.ingest(ctx, nextLedgerSeq); err != nil {
			return err
		}
	}
	return ctx.Err()
}

func (r *Runner) maybeFillEntriesFromCheckpoint(ctx context.Context, archive historyarchive.ArchiveInterface) (uint32, chan error, error) {
	checkPointFillErr := make(chan error, 1)
	// First, make sure the DB has a complete ledger entry baseline
	curLedgerSeq, err := r.db.GetLatestLedgerSequence(ctx)
	if err == db.ErrEmptyDB {
		var checkpointLedger uint32
		if root, rootErr := archive.GetRootHAS(); rootErr != nil {
			return 0, checkPointFillErr, rootErr
		} else {
			checkpointLedger = root.CurrentLedger
		}

		// DB is empty, let's fill it from the History Archive, using the latest available checkpoint
		// Do it in parallel with the upcoming captive core preparation to save time
		r.logger.Infof("Found an empty database, filling it in from the most recent checkpoint (this can take up to 30 minutes, depending on the network)")
		go func() {
			checkPointFillErr <- r.fillEntriesFromCheckpoint(ctx, archive, checkpointLedger)
		}()
		return checkpointLedger + 1, checkPointFillErr, nil
	} else if err != nil {
		return 0, checkPointFillErr, err
	} else {
		checkPointFillErr <- nil
		return curLedgerSeq + 1, checkPointFillErr, nil
	}
}

func (r *Runner) fillEntriesFromCheckpoint(ctx context.Context, archive historyarchive.ArchiveInterface, checkpointLedger uint32) error {
	r.logger.Infof("Starting processing of checkpoint %d", checkpointLedger)
	checkpointCtx, cancelCheckpointCtx := context.WithTimeout(ctx, r.timeout)
	defer cancelCheckpointCtx()

	reader, err := ingest.NewCheckpointChangeReader(checkpointCtx, archive, checkpointLedger)
	if err != nil {
		return err
	}

	tx, err := r.db.NewTx(ctx)
	if err != nil {
		return err
	}
	defer func() {
		if err := tx.Rollback(); err != nil {
			r.logger.WithError(err).Warn("could not rollback fillEntriesFromCheckpoint write transactions")
		}
	}()

	if err := r.ingestLedgerEntryChanges(ctx, reader, tx); err != nil {
		return err
	}
	if err := reader.Close(); err != nil {
		return err
	}

	r.logger.Info("Committing checkpoint ledger entries")
	if err := tx.Commit(checkpointLedger); err != nil {
		return err
	}
	if err := r.db.WALCheckpoint(ctx); err != nil {
		return err
	}
	r.logger.Info("Finished checkpoint processing")
	return nil
}

func (r *Runner) ingest(ctx context.Context, sequence uint32) error {
	r.logger.Infof("Applying txmeta ledger entries changes for ledger %d", sequence)
	ledgerCloseMeta, err := r.ledgerBackend.GetLedger(ctx, sequence)
	if err != nil {
		return err
	}
	reader, err := ingest.NewLedgerChangeReaderFromLedgerCloseMeta(r.networkPassPhrase, ledgerCloseMeta)
	if err != nil {
		return err
	}
	tx, err := r.db.NewTx(ctx)
	if err != nil {
		return err
	}
	defer func() {
		if err := tx.Rollback(); err != nil {
			r.logger.WithError(err).Warn("could not rollback ingest write transactions")
		}
	}()

	if err := r.ingestLedgerEntryChanges(ctx, reader, tx); err != nil {
		return err
	}
	if err := reader.Close(); err != nil {
		return err
	}

	if err := r.ingestLedgerCloseMeta(tx, ledgerCloseMeta); err != nil {
		return err
	}

	if err := tx.Commit(sequence); err != nil {
		return err
	}
	if err := r.db.WALCheckpoint(ctx); err != nil {
		return err
	}
	return nil
}

func (r *Runner) ingestLedgerCloseMeta(tx db.WriteTx, ledgerCloseMeta xdr.LedgerCloseMeta) error {
	ledgerWriter := tx.LedgerWriter()
	if err := ledgerWriter.InsertLedger(ledgerCloseMeta); err != nil {
		return err
	}
	return nil
}
