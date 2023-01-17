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

const (
	maxBatchSize                      = 150
	checkpointLedgerEntryPrintoutFreq = 10000
)

type LedgerEntryStorage interface {
	GetLedgerEntry(key xdr.LedgerKey) (xdr.LedgerEntry, bool, uint32, error)
	io.Closer
}

type LedgerEntryStorageCfg struct {
	Logger            *log.Entry
	DB                DB
	NetworkPassPhrase string
	Archive           historyarchive.ArchiveInterface
	LedgerBackend     backends.LedgerBackend
	Timeout           time.Duration
}

func NewLedgerEntryStorage(cfg LedgerEntryStorageCfg) (LedgerEntryStorage, error) {
	ctx, done := context.WithCancel(context.Background())
	ls := ledgerEntryStorage{
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

type ledgerEntryStorage struct {
	logger            *log.Entry
	db                DB
	timeout           time.Duration
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
	return ls.db.Close()
}

func (ls *ledgerEntryStorage) fillEntriesFromLatestCheckpoint(ctx context.Context, archive historyarchive.ArchiveInterface) (uint32, error) {
	root, err := archive.GetRootHAS()
	if err != nil {
		return 0, err
	}
	startCheckpointLedger := root.CurrentLedger

	ls.logger.Infof("Starting processing of checkpoint %d", startCheckpointLedger)
	checkpointCtx, cancelCheckpointCtx := context.WithTimeout(ctx, ls.timeout)
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
			return 0, context.Canceled
		default:
		}
		change, err := reader.Read()
		if err == io.EOF {
			break
		}
		if err != nil {
			return 0, err
		}

		entry := change.Post
		key, relevant := getRelevantLedgerKeyFromData(entry.Data)
		if !relevant {
			continue
		}
		if err := tx.UpsertLedgerEntry(key, *entry); err != nil {
			return 0, err
		}
		entryCount++

		if entryCount%checkpointLedgerEntryPrintoutFreq == 0 {
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
	if err == ErrEmptyDB {
		// DB is empty, let's fill it from a checkpoint
		ls.logger.Infof("Found an empty database, filling it in from the most recent checkpoint (this can take up to 30 minutes, depending on the network)")
		startCheckpointLedger, err = ls.fillEntriesFromLatestCheckpoint(ctx, archive)
		if err != nil {
			panic(err)
		}
	}
	if err != nil {
		panic(err)
	}

	// Secondly, continuously process txmeta deltas

	// TODO: we can probably do the preparation in parallel with the checkpoint processing above
	prepareRangeCtx, cancelPrepareRange := context.WithTimeout(ctx, ls.timeout)
	if err := ledgerBackend.PrepareRange(prepareRangeCtx, backends.UnboundedRange(startCheckpointLedger)); err != nil {
		panic(err)
	}
	cancelPrepareRange()

	nextLedger := startCheckpointLedger + 1
	for {
		fmt.Println("Processing txmeta of ledger", nextLedger)
		reader, err := ingest.NewLedgerChangeReader(ctx, ledgerBackend, ls.networkPassPhrase, nextLedger)
		if err != nil {
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
		if err := key.SetContractData(data.ContractData.ContractId, data.ContractData.Val); err != nil {
			panic(err)
		}
	default:
		// we don't care about any other entry types for now
		return xdr.LedgerKey{}, false
	}
	return key, true
}
