package transactions

import (
	"github.com/stellar/go/ingest"
	"sync"

	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/ledgerbucketwindow"
)

type transaction struct {
	bucket           *ledgerbucketwindow.LedgerBucket[[]xdr.Hash]
	result           xdr.TransactionResult
	meta             xdr.TransactionMeta
	envelope         xdr.TransactionEnvelope
	feeBump          bool
	applicationOrder int32
}

// MemoryStore is an in-memory store of Stellar transactions.
type MemoryStore struct {
	// networkPassphrase is an immutable string containing the
	// Stellar network passphrase.
	// Accessing networkPassphrase does not need to be protected
	// by the lock
	networkPassphrase    string
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
func NewMemoryStore(networkPassphrase string, retentionWindow uint32) (*MemoryStore, error) {
	window, err := ledgerbucketwindow.NewLedgerBucketWindow[[]xdr.Hash](retentionWindow)
	if err != nil {
		return nil, err
	}
	return &MemoryStore{
		networkPassphrase:    networkPassphrase,
		transactions:         make(map[xdr.Hash]transaction),
		transactionsByLedger: window,
	}, nil
}

// IngestTransactions adds new transactions from the given ledger into the store.
// As a side effect, transactions which fall outside the retention window are
// removed from the store.
func (m *MemoryStore) IngestTransactions(ledgerCloseMeta xdr.LedgerCloseMeta) error {
	reader, err := ingest.NewLedgerTransactionReaderFromLedgerCloseMeta(m.networkPassphrase, ledgerCloseMeta)
	if err != nil {
		return err
	}

	txCount := ledgerCloseMeta.CountTransactions()
	transactions := make([]transaction, txCount)
	hashes := make([]xdr.Hash, 0, txCount)
	hashMap := map[xdr.Hash]transaction{}
	var bucket ledgerbucketwindow.LedgerBucket[[]xdr.Hash]

	for i := 0; i < txCount; i++ {
		tx, err := reader.Read()
		if err != nil {
			return err
		}
		transactions[i] = transaction{
			bucket:           &bucket,
			result:           tx.Result.Result,
			meta:             tx.UnsafeMeta,
			envelope:         tx.Envelope,
			feeBump:          tx.Envelope.IsFeeBump(),
			applicationOrder: int32(tx.Index),
		}
		if transactions[i].feeBump {
			innerHash := tx.Result.InnerHash()
			hashMap[innerHash] = transactions[i]
			hashes = append(hashes, innerHash)
		}
		hashMap[tx.Result.TransactionHash] = transactions[i]
		hashes = append(hashes, tx.Result.TransactionHash)
	}
	bucket = ledgerbucketwindow.LedgerBucket[[]xdr.Hash]{
		LedgerSeq:            ledgerCloseMeta.LedgerSequence(),
		LedgerCloseTimestamp: int64(ledgerCloseMeta.LedgerHeaderHistoryEntry().Header.ScpValue.CloseTime),
		BucketContent:        hashes,
	}

	m.lock.Lock()
	defer m.lock.Unlock()
	evicted := m.transactionsByLedger.Append(bucket)
	if evicted != nil {
		// garbage-collect evicted entries
		for _, evictedTxHash := range evicted.BucketContent {
			delete(m.transactions, evictedTxHash)
		}
	}
	for hash, tx := range hashMap {
		m.transactions[hash] = tx
	}
	return nil
}

type LedgerInfo struct {
	Sequence  uint32
	CloseTime int64
}

type Transaction struct {
	Result           xdr.TransactionResult
	Meta             xdr.TransactionMeta
	Envelope         xdr.TransactionEnvelope
	FeeBump          bool
	ApplicationOrder int32
	Ledger           LedgerInfo
}

type StoreRange struct {
	FirstLedger LedgerInfo
	LastLedger  LedgerInfo
}

// GetLatestLedger returns the latest ledger available in the store.
func (m *MemoryStore) GetLatestLedger() LedgerInfo {
	m.lock.RLock()
	defer m.lock.RUnlock()
	if m.transactionsByLedger.Len() > 0 {
		lastBucket := m.transactionsByLedger.Get(m.transactionsByLedger.Len() - 1)
		return LedgerInfo{
			Sequence:  lastBucket.LedgerSeq,
			CloseTime: lastBucket.LedgerCloseTimestamp,
		}
	}
	return LedgerInfo{}
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
		Meta:             internalTx.meta,
		Envelope:         internalTx.envelope,
		FeeBump:          internalTx.feeBump,
		ApplicationOrder: internalTx.applicationOrder,
		Ledger: LedgerInfo{
			Sequence:  internalTx.bucket.LedgerSeq,
			CloseTime: internalTx.bucket.LedgerCloseTimestamp,
		},
	}
	return tx, true, storeRange
}
