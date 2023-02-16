package transactions

import (
	"sync"

	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
)

type Transaction struct {
	id       xdr.Hash
	envelope xdr.TransactionEnvelope
	result   xdr.TransactionResult
	meta     xdr.TransactionMeta
}

// MemoryStore is an in-memory store of Stellar transactions.
type MemoryStore struct {
	lock                 sync.RWMutex
	transactions         map[xdr.Hash]Transaction
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
		transactions:         make(map[xdr.Hash]Transaction),
		transactionsByLedger: window,
	}, nil
}

// IngestTransactions adds new transactions from the given ledger into the store.
// As a side effect, transactions which fall outside the retention window are
// removed from the store.
func (m *MemoryStore) IngestTransactions(ledgerCloseMeta xdr.LedgerCloseMeta) error {
	transactions := readTransactions(ledgerCloseMeta)
	ledgerSequence := ledgerCloseMeta.LedgerSequence()
	ledgerCloseTime := int64(ledgerCloseMeta.LedgerHeaderHistoryEntry().Header.ScpValue.CloseTime)
	m.lock.Lock()
	defer m.lock.Unlock()
	transactionHashes := make([]xdr.Hash, len(transactions))
	for i, tx := range transactions {
		m.transactions[tx.id] = tx
		transactionHashes[i] = transactions[i].id
	}
	evicted, err := m.transactionsByLedger.Append(ledgerSequence, ledgerCloseTime, transactionHashes)
	if evicted != nil {
		// garbage-collect evicted entries
		for _, evictedTxHash := range evicted.BucketContent {
			delete(m.transactions, evictedTxHash)
		}
	}
	return err
}

func readTransactions(ledgerCloseMeta xdr.LedgerCloseMeta) []Transaction {
	envs := ledgerCloseMeta.TransactionEnvelopes()
	result := make([]Transaction, len(envs))
	for i := range envs {
		resultPair := ledgerCloseMeta.TransactionResultPair(i)
		result[i].id = resultPair.TransactionHash
		result[i].envelope = envs[i]
		result[i].result = resultPair.Result
		result[i].meta = ledgerCloseMeta.TxApplyProcessing(i)
	}
	return result
}

// GetTransaction obtains a transaction from the store and whether it's present.
func (m *MemoryStore) GetTransaction(hash xdr.Hash) (Transaction, bool) {
	m.lock.RLock()
	tx, ok := m.transactions[hash]
	m.lock.RUnlock()
	return tx, ok
}
