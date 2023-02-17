package transactions

import (
	"sync"

	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
)

type transaction struct {
	bucket           *ledgerbucketwindow.LedgerBucket[[]xdr.Hash]
	result           xdr.TransactionResult
	applicationOrder int32
}

// MemoryStore is an in-memory store of Stellar transactions.
type MemoryStore struct {
	lock                 sync.RWMutex
	transactions         map[xdr.Hash]transaction
	transactionsByLedger *ledgerbucketwindow.LedgerBucketWindow[[]xdr.Hash]
}

// NewMemoryStore creates a new MemoryStore.
// The retention window is in units of ledgers.
// All events occurring in the following ledger range
// [ latestLedger - retentionWindow, latestLedger ]
// will be included in the MemoryStore. If the MemoryStore
// is full, any transactions from new ledgers will evict
// older entries outside the retention window.
func NewMemoryStore(retentionWindow uint32) (*MemoryStore, error) {
	window, err := ledgerbucketwindow.NewLedgerBucketWindow[[]xdr.Hash](retentionWindow)
	if err != nil {
		return nil, err
	}
	return &MemoryStore{
		transactions:         make(map[xdr.Hash]transaction),
		transactionsByLedger: window,
	}, nil
}

// IngestTransactions adds new transactions from the given ledger into the store.
// As a side effect, transactions which fall outside the retention window are
// removed from the store.
func (m *MemoryStore) IngestTransactions(ledgerCloseMeta xdr.LedgerCloseMeta) error {
	txCount := ledgerCloseMeta.CountTransactions()
	transactions := make([]transaction, txCount)
	transactionHashes := make([]xdr.Hash, txCount)
	var bucket ledgerbucketwindow.LedgerBucket[[]xdr.Hash]
	for i := 0; i < txCount; i++ {
		resultPair := ledgerCloseMeta.TransactionResultPair(i)
		transactionHashes[i] = resultPair.TransactionHash
		transactions[i].result = resultPair.Result
		transactions[i].applicationOrder = int32(i) + 1 // Transactions start at '1'
		transactions[i].bucket = &bucket
	}
	bucket = ledgerbucketwindow.LedgerBucket[[]xdr.Hash]{
		LedgerSeq:            ledgerCloseMeta.LedgerSequence(),
		LedgerCloseTimestamp: int64(ledgerCloseMeta.LedgerHeaderHistoryEntry().Header.ScpValue.CloseTime),
		BucketContent:        transactionHashes,
	}

	m.lock.Lock()
	defer m.lock.Unlock()
	evicted, err := m.transactionsByLedger.Append(bucket)
	if err != nil {
		return err
	}
	if evicted != nil {
		// garbage-collect evicted entries
		for _, evictedTxHash := range evicted.BucketContent {
			delete(m.transactions, evictedTxHash)
		}
	}
	for i := range transactions {
		m.transactions[transactionHashes[i]] = transactions[i]
	}
	return nil
}

type LedgerInfo struct {
	Sequence  uint32
	CloseTime int64
}

type Transaction struct {
	Result           xdr.TransactionResult
	ApplicationOrder int32
	Ledger           LedgerInfo
}

type StoreRange struct {
	FirstLedger LedgerInfo
	LastLedger  LedgerInfo
}

// GetTransaction obtains a transaction from the store and whether it's present and the current store range
func (m *MemoryStore) GetTransaction(hash xdr.Hash) (Transaction, bool, StoreRange) {
	m.lock.RLock()
	defer m.lock.RUnlock()
	var storeRange StoreRange
	if m.transactionsByLedger.Len() > 0 {
		firstBucket := m.transactionsByLedger.Get(0)
		lastBucket := m.transactionsByLedger.Get(m.transactionsByLedger.Len() - 1)
		storeRange = StoreRange{
			FirstLedger: LedgerInfo{
				Sequence:  firstBucket.LedgerSeq,
				CloseTime: firstBucket.LedgerCloseTimestamp,
			},
			LastLedger: LedgerInfo{
				Sequence:  lastBucket.LedgerSeq,
				CloseTime: lastBucket.LedgerCloseTimestamp,
			},
		}
	}
	internalTx, ok := m.transactions[hash]
	if !ok {
		return Transaction{}, false, storeRange
	}
	tx := Transaction{
		Result:           internalTx.result,
		ApplicationOrder: internalTx.applicationOrder,
		Ledger: LedgerInfo{
			Sequence:  internalTx.bucket.LedgerSeq,
			CloseTime: internalTx.bucket.LedgerCloseTimestamp,
		},
	}
	return tx, true, storeRange
}
