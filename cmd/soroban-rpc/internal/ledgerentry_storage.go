package internal

import (
	"context"
	"errors"
	"fmt"
	"io"
	"sync"
	"time"

	"github.com/stellar/go/historyarchive"
	"github.com/stellar/go/ingest"
	backends "github.com/stellar/go/ingest/ledgerbackend"
	"github.com/stellar/go/xdr"
)

type LedgerEntryStorage interface {
	GetLedgerEntry(key xdr.LedgerKey) (xdr.LedgerEntry, bool, uint32, error)
	io.Closer
}

func NewLedgerEntryStorage(
	networkPassPhrase string,
	archive historyarchive.ArchiveInterface,
	ledgerBackend backends.LedgerBackend) (LedgerEntryStorage, error) {
	root, err := archive.GetRootHAS()
	if err != nil {
		return nil, err
	}
	checkpointLedger := root.CurrentLedger
	ctx, done := context.WithCancel(context.Background())
	ls := ledgerEntryStorage{
		networkPassPhrase: networkPassPhrase,
		storage:           map[string]xdr.LedgerEntry{},
		done:              done,
	}
	ls.wg.Add(1)
	go ls.run(ctx, checkpointLedger, archive, ledgerBackend)
	return &ls, nil
}

type ledgerEntryStorage struct {
	encodingBuffer    *xdr.EncodingBuffer
	networkPassPhrase string
	// from serialized ledger key to ledger entry
	storage map[string]xdr.LedgerEntry
	// What's the latest processed ledger
	latestLedger uint32
	sync.RWMutex
	done context.CancelFunc
	wg   sync.WaitGroup
}

func (ls *ledgerEntryStorage) GetLedgerEntry(key xdr.LedgerKey) (xdr.LedgerEntry, bool, uint32, error) {
	stringKey := getRelevantLedgerKey(ls.encodingBuffer, key)
	if stringKey == "" {
		return xdr.LedgerEntry{}, false, 0, nil
	}
	ls.RLock()
	defer ls.RUnlock()
	if ls.latestLedger == 0 {
		// we haven't yet processed the first checkpoint
		return xdr.LedgerEntry{}, false, 0, errors.New("Ledger storage not initialized yet")
	}

	entry, present := ls.storage[stringKey]
	if !present {
		return xdr.LedgerEntry{}, false, 0, nil
	}
	return entry, true, ls.latestLedger, nil
}

func (ls *ledgerEntryStorage) Close() error {
	ls.done()
	ls.wg.Wait()
	return nil
}

func (ls *ledgerEntryStorage) run(ctx context.Context, startCheckpointLedger uint32, archive historyarchive.ArchiveInterface, ledgerBackend backends.LedgerBackend) {
	defer ls.wg.Done()

	// First, process the checkpoint
	// TODO: use a logger
	fmt.Println("Starting processing of checkpoint", startCheckpointLedger)
	checkpointCtx, cancelCheckpointCtx := context.WithTimeout(ctx, 30*time.Minute)
	reader, err := ingest.NewCheckpointChangeReader(checkpointCtx, archive, startCheckpointLedger)
	if err != nil {
		// TODO: implement retries instead
		panic(err)
	}
	// We intentionally use this local encoding buffer to avoid race conditions with the main one
	buffer := xdr.NewEncodingBuffer()

	for {
		select {
		case <-ctx.Done():
			cancelCheckpointCtx()
			return
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
		key := getRelevantLedgerKeyFromData(buffer, entry.Data)
		if key == "" {
			// not relevant
			continue
		}

		// no need to Write-lock until we process the full checkpoint, since the reader checks latestLedger to be non-zero
		ls.storage[key] = *entry

		if len(ls.storage)%2000 == 0 {
			fmt.Printf("  processed %d checkpoint ledger entries\n", len(ls.storage))
		}
	}

	cancelCheckpointCtx()

	fmt.Println("Finished checkpoint processing")
	ls.Lock()
	ls.latestLedger = startCheckpointLedger
	ls.Unlock()

	// Now, continuously process txmeta deltas

	// TODO: we can probably do the preparation in parallel with the checkpoint processing
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

		// TODO: completely blocking reads between ledgers being processed may not be acceptable
		//       however, we don't want to return ledger entries inbetween ledger updates
		ls.Lock()
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
				key := getRelevantLedgerKeyFromData(buffer, change.Pre.Data)
				if key == "" {
					continue
				}
				delete(ls.storage, key)
			} else {
				key := getRelevantLedgerKeyFromData(buffer, change.Post.Data)
				if key == "" {
					continue
				}
				ls.storage[key] = *change.Post
			}
		}
		ls.latestLedger = nextLedger
		nextLedger++
		fmt.Println("Ledger entry count", len(ls.storage))
		ls.Unlock()
		reader.Close()
	}

}

func getRelevantLedgerKey(buffer *xdr.EncodingBuffer, key xdr.LedgerKey) string {
	// this is safe since we are converting to string right away, which causes a copy
	binKey, err := buffer.LedgerKeyUnsafeMarshalBinaryCompress(key)
	if err != nil {
		// TODO: we probably don't want to panic
		panic(err)
	}
	return string(binKey)
}

func getRelevantLedgerKeyFromData(buffer *xdr.EncodingBuffer, data xdr.LedgerEntryData) string {
	var key xdr.LedgerKey
	switch data.Type {
	case xdr.LedgerEntryTypeAccount:
		key.SetAccount(data.Account.AccountId)
	case xdr.LedgerEntryTypeTrustline:
		key.SetTrustline(data.TrustLine.AccountId, data.TrustLine.Asset)
	case xdr.LedgerEntryTypeContractData:
		key.SetContractData(data.ContractData.ContractId, data.ContractData.Val)
	default:
		// we don't care about any other entry types for now
		return ""
	}
	return getRelevantLedgerKey(buffer, key)
}
