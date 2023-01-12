package ledgerentry_storage

import (
	"context"
	"fmt"
	"io"
	"sync"
	"time"

	"github.com/stellar/go/historyarchive"
	"github.com/stellar/go/ingest"
	backends "github.com/stellar/go/ingest/ledgerbackend"
	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"
)

// TODO: Make this configurable?
const maxBatchSize = 150

type LedgerEntryStorage interface {
	GetLedgerEntry(key xdr.LedgerKey) (xdr.LedgerEntry, bool, uint32, error)
	io.Closer
}

func NewLedgerEntryStorage(logger *log.Entry, db DB, networkPassPhrase string, archive historyarchive.ArchiveInterface, ledgerBackend backends.LedgerBackend) (LedgerEntryStorage, error) {
	ctx, done := context.WithCancel(context.Background())
	ls := ledgerEntryStorage{
		logger:            logger,
		db:                db,
		networkPassPhrase: networkPassPhrase,
		done:              done,
	}
	ls.wg.Add(1)
	go ls.run(ctx, archive, ledgerBackend)
	return &ls, nil
}

type ledgerEntryStorage struct {
	logger            *log.Entry
	db                DB
	networkPassPhrase string
	done              context.CancelFunc
	wg                sync.WaitGroup
}

func (ls *ledgerEntryStorage) GetLedgerEntry(key xdr.LedgerKey) (xdr.LedgerEntry, bool, uint32, error) {
	return ls.db.GetLedgerEntry(key)
}

func (ls *ledgerEntryStorage) Close() error {
	ls.done()
	ls.wg.Wait()
	ls.db.Close()
	return nil
}

func (ls *ledgerEntryStorage) fillEntriesFromLatestCheckpoint(ctx context.Context, archive historyarchive.ArchiveInterface) (uint32, error) {
	root, err := archive.GetRootHAS()
	if err != nil {
		return 0, err
	}
	startCheckpointLedger := root.CurrentLedger

	ls.logger.Infof("Starting processing of checkpoint %d", startCheckpointLedger)
	// TODO: should we make this configurable?
	checkpointCtx, cancelCheckpointCtx := context.WithTimeout(ctx, 30*time.Minute)
	defer cancelCheckpointCtx()
	reader, err := ingest.NewCheckpointChangeReader(checkpointCtx, archive, startCheckpointLedger)
	if err != nil {
		return 0, err
	}
	tx, err := ls.db.NewLedgerEntryUpdaterTx(startCheckpointLedger, maxBatchSize)
	if err != nil {
		return 0, err
	}
	// Make sure we finish the updating transaction
	entryCount := 0

	for {
		select {
		case <-ctx.Done():
			cancelCheckpointCtx()
			return 0, context.Canceled
		default:
		}
		change, err := reader.Read()
		if err == io.EOF {
			break
		}
		if err != nil {
			// TODO: we probably shouldn't panic, at least in case of timeout
			panic(err)
		}

		entry := change.Post
		key, relevant, err := getRelevantLedgerKeyFromData(entry.Data)
		if err != nil {
			return 0, err
		}
		if !relevant {
			continue
		}
		tx.UpsertLedgerEntry(key, *entry)
		entryCount++

		if entryCount%10000 == 0 {
			ls.logger.Infof("  processed %d checkpoint ledger entry changes", entryCount)
		}
	}

	ls.logger.Info("Committing ledger entries")
	if err = tx.Done(); err != nil {
		return 0, err
	}

	ls.logger.Info("Finished checkpoint processing")
	return startCheckpointLedger, nil
}

func (ls *ledgerEntryStorage) run(ctx context.Context, archive historyarchive.ArchiveInterface, ledgerBackend backends.LedgerBackend) {
	defer ls.wg.Done()

	// First, make sure the DB has a complete ledger entry baseline

	startCheckpointLedger, err := ls.db.GetLatestLedgerSequence()
	if err != nil && err != ErrEmptyDB {
		// TODO: implement retries?
		panic(err)
	}
	if err == ErrEmptyDB {
		// DB is empty, let's fill it from a checkpoint
		ls.logger.Infof("Found an empty database, filling it in from the most recent checkpoint (this can take up to 30 minutes, depending on the network)")
		startCheckpointLedger, err = ls.fillEntriesFromLatestCheckpoint(ctx, archive)
		// TODO: implement retries?
		if err != nil {
			panic(err)
		}
	}

	// Secondly, continuously process txmeta deltas

	// TODO: we can probably do the preparation in parallel with the checkpoint processing above
	prepareRangeCtx, cancelPrepareRange := context.WithTimeout(ctx, 30*time.Minute)
	if err := ledgerBackend.PrepareRange(prepareRangeCtx, backends.UnboundedRange(startCheckpointLedger)); err != nil {
		// TODO: we probably shouldn't panic, at least in case of timeout
		panic(err)
	}
	cancelPrepareRange()

	nextLedger := startCheckpointLedger + 1
	for {
		fmt.Println("Processing txmeta of ledger", nextLedger)
		reader, err := ingest.NewLedgerChangeReader(ctx, ledgerBackend, ls.networkPassPhrase, nextLedger)
		if err != nil {
			// TODO: we probably shouldn't panic, at least in case of timeout/cancellation
			panic(err)
		}
		tx, err := ls.db.NewLedgerEntryUpdaterTx(nextLedger, maxBatchSize)
		if err != nil {
			// TODO: we probably shouldn't panic, at least in case of timeout/cancellation
			panic(err)
		}

		for {
			change, err := reader.Read()
			if err == io.EOF {
				break
			}
			if err != nil {
				// TODO: we probably shouldn't panic, at least in case of timeout/cancellation
				panic(err)
			}
			if change.Post == nil {
				key, relevant, err := getRelevantLedgerKeyFromData(change.Pre.Data)
				if err != nil {
					// TODO: we probably shouldn't panic, at least in case of timeout/cancellation
					panic(err)
				}
				if !relevant {
					continue
				}
				if err != nil {
					// TODO: we probably shouldn't panic, at least in case of timeout/cancellation
					panic(err)
				}
				err = tx.DeleteLedgerEntry(key)
				if err != nil {
					// TODO: we probably shouldn't panic, at least in case of timeout/cancellation
					panic(err)
				}
			} else {
				key, relevant, err := getRelevantLedgerKeyFromData(change.Post.Data)
				if err != nil {
					// TODO: we probably shouldn't panic, at least in case of timeout/cancellation
					panic(err)
				}
				if !relevant {
					continue
				}
				if err != nil {
					// TODO: we probably shouldn't panic, at least in case of timeout/cancellation
					panic(err)
				}
				err = tx.UpsertLedgerEntry(key, *change.Post)
				if err != nil {
					// TODO: we probably shouldn't panic, at least in case of timeout/cancellation
					panic(err)
				}
			}
		}
		tx.Done()
		nextLedger++
		reader.Close()
	}

}

func getRelevantLedgerKeyFromData(data xdr.LedgerEntryData) (xdr.LedgerKey, bool, error) {
	var key xdr.LedgerKey
	switch data.Type {
	case xdr.LedgerEntryTypeAccount:
		if err := key.SetAccount(data.Account.AccountId); err != nil {
			return xdr.LedgerKey{}, false, err
		}
	case xdr.LedgerEntryTypeTrustline:
		if err := key.SetTrustline(data.TrustLine.AccountId, data.TrustLine.Asset); err != nil {
			return xdr.LedgerKey{}, false, err
		}
	case xdr.LedgerEntryTypeContractData:
		if err := key.SetContractData(data.ContractData.ContractId, data.ContractData.Val); err != nil {
			return xdr.LedgerKey{}, false, err
		}
	default:
		// we don't care about any other entry types for now
		return xdr.LedgerKey{}, false, nil
	}
	return key, true, nil
}
