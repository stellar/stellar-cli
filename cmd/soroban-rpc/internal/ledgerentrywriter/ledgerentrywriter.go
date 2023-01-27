package ledgerentrywriter

import (
	"context"
	"errors"
	"io"
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
	maxBatchSize                      = 150
	checkpointLedgerEntryPrintoutFreq = 10000
)

type LedgerEntryWriterCfg struct {
	Logger            *log.Entry
	DB                db.DB
	NetworkPassPhrase string
	Archive           historyarchive.ArchiveInterface
	LedgerBackend     backends.LedgerBackend
	Timeout           time.Duration
}

func NewLedgerEntryWriter(cfg LedgerEntryWriterCfg) (*LedgerEntryWriter, error) {
	ctx, done := context.WithCancel(context.Background())
	ls := LedgerEntryWriter{
		logger:            cfg.Logger,
		db:                cfg.DB,
		networkPassPhrase: cfg.NetworkPassPhrase,
		timeout:           cfg.Timeout,
		done:              done,
	}
	ls.wg.Add(1)
	go ls.run(ctx, cfg.Archive, cfg.LedgerBackend)
	return &ls, nil
}

type LedgerEntryWriter struct {
	logger            *log.Entry
	db                db.DB
	timeout           time.Duration
	networkPassPhrase string
	done              context.CancelFunc
	wg                sync.WaitGroup
}

func (ls *LedgerEntryWriter) Close() error {
	ls.done()
	ls.wg.Wait()
	return nil
}

func (ls *LedgerEntryWriter) fillEntriesFromCheckpoint(ctx context.Context, archive historyarchive.ArchiveInterface, checkpointLedger uint32) error {
	ls.logger.Infof("Starting processing of checkpoint %d", checkpointLedger)
	checkpointCtx, cancelCheckpointCtx := context.WithTimeout(ctx, ls.timeout)
	defer cancelCheckpointCtx()
	reader, err := ingest.NewCheckpointChangeReader(checkpointCtx, archive, checkpointLedger)
	if err != nil {
		return err
	}
	tx, err := ls.db.NewLedgerEntryUpdaterTx(checkpointLedger, maxBatchSize)
	if err != nil {
		return err
	}
	// Make sure we finish the updating transaction
	entryCount := 0

	for {
		select {
		case <-ctx.Done():
			return context.Canceled
		default:
		}
		change, err := reader.Read()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}

		entry := change.Post
		key, relevant := getRelevantLedgerKeyFromData(entry.Data)
		if !relevant {
			continue
		}
		if err := tx.UpsertLedgerEntry(key, *entry); err != nil {
			return err
		}
		entryCount++

		if entryCount%checkpointLedgerEntryPrintoutFreq == 0 {
			ls.logger.Infof("  processed %d checkpoint ledger entry changes", entryCount)
		}
	}

	ls.logger.Info("Committing checkpoint ledger entries")
	if err = tx.Done(); err != nil {
		return err
	}

	ls.logger.Info("Finished checkpoint processing")
	return nil
}

func (ls *LedgerEntryWriter) run(ctx context.Context, archive historyarchive.ArchiveInterface, ledgerBackend backends.LedgerBackend) {
	defer ls.wg.Done()
	var checkPointPrefillWg sync.WaitGroup

	// First, make sure the DB has a complete ledger entry baseline
	startLedger, err := db.GetLatestLedgerSequence(ls.db)
	if err == db.ErrEmptyDB {
		// DB is empty, let's fill it from the History Archive, using the latest available checkpoint
		ls.logger.Infof("Found an empty database, filling it in from the most recent checkpoint (this can take up to 30 minutes, depending on the network)")
		root, err := archive.GetRootHAS()
		if err != nil {
			panic(err)
		}
		startLedger = root.CurrentLedger
		// Do it in parallel with the upcoming captive core preparation to save time
		checkPointPrefillWg.Add(1)
		go func() {
			defer checkPointPrefillWg.Done()
			if err = ls.fillEntriesFromCheckpoint(ctx, archive, startLedger); err != nil {
				if errors.Is(err, context.Canceled) {
					// we were told to stop
					return
				}
				panic(err)
			}
		}()
	} else if err != nil {
		panic(err)
	}

	// Secondly, continuously process txmeta deltas
	prepareRangeCtx, cancelPrepareRange := context.WithTimeout(ctx, ls.timeout)
	if err := ledgerBackend.PrepareRange(prepareRangeCtx, backends.UnboundedRange(startLedger)); err != nil {
		if errors.Is(err, context.Canceled) {
			// we were told to stop
			return
		}
		panic(err)
	}
	cancelPrepareRange()

	// Make sure that the checkpoint prefill (if any), happened before starting to apply deltas
	checkPointPrefillWg.Wait()

	nextLedger := startLedger + 1
	for {
		ls.logger.Infof("Applying txmeta ledger entries changes for ledger %d", nextLedger)
		reader, err := ingest.NewLedgerChangeReader(ctx, ledgerBackend, ls.networkPassPhrase, nextLedger)
		if err != nil {
			if errors.Is(err, context.Canceled) {
				// we were told to stop
				return
			}
			panic(err)
		}
		tx, err := ls.db.NewLedgerEntryUpdaterTx(nextLedger, maxBatchSize)
		if err != nil {
			panic(err)
		}

		for {
			change, err := reader.Read()
			if err == io.EOF {
				break
			}
			if err != nil {
				panic(err)
			}
			if change.Post == nil {
				key, relevant := getRelevantLedgerKeyFromData(change.Pre.Data)
				if !relevant {
					continue
				}

				if err := tx.DeleteLedgerEntry(key); err != nil {
					panic(err)
				}
			} else {
				key, relevant := getRelevantLedgerKeyFromData(change.Post.Data)
				if !relevant {
					continue
				}

				if err := tx.UpsertLedgerEntry(key, *change.Post); err != nil {
					panic(err)
				}
			}
		}
		if err := tx.Done(); err != nil {
			panic(err)
		}
		nextLedger++
		if err := reader.Close(); err != nil {
			panic(err)
		}
	}
}

func getRelevantLedgerKeyFromData(data xdr.LedgerEntryData) (xdr.LedgerKey, bool) {
	var key xdr.LedgerKey
	switch data.Type {
	case xdr.LedgerEntryTypeAccount:
		if err := key.SetAccount(data.Account.AccountId); err != nil {
			panic(err)
		}
	case xdr.LedgerEntryTypeTrustline:
		if err := key.SetTrustline(data.TrustLine.AccountId, data.TrustLine.Asset); err != nil {
			panic(err)
		}
	case xdr.LedgerEntryTypeContractData:
		if err := key.SetContractData(data.ContractData.ContractId, data.ContractData.Key); err != nil {
			panic(err)
		}
	case xdr.LedgerEntryTypeContractCode:
		if err := key.SetContractCode(data.ContractCode.Hash); err != nil {
			panic(err)
		}
	default:
		// we don't care about any other entry types for now
		return xdr.LedgerKey{}, false
	}
	return key, true
}
