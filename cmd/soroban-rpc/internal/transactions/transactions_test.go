package transactions

import (
	"testing"

	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/require"
)

func expectedTransaction(ledger uint32) Transaction {
	return Transaction{
		Result:           transactionResult(ledger),
		ApplicationOrder: 1,
		Ledger:           expectedLedgerInfo(ledger),
	}
}

func expectedLedgerInfo(ledgerSequence uint32) LedgerInfo {
	return LedgerInfo{
		Sequence:  ledgerSequence,
		CloseTime: ledgerCloseTime(ledgerSequence),
	}

}

func expectedStoreRange(startLedger uint32, endLedger uint32) StoreRange {
	return StoreRange{
		FirstLedger: expectedLedgerInfo(startLedger),
		LastLedger:  expectedLedgerInfo(endLedger),
	}
}

func txHash(ledgerSequence uint32) xdr.Hash {
	return xdr.Hash{byte(ledgerSequence), byte(ledgerSequence)}
}

func ledgerCloseTime(ledgerSequence uint32) int64 {
	return int64(ledgerSequence)*25 + 100
}

func transactionResult(ledgerSequence uint32) xdr.TransactionResult {
	return xdr.TransactionResult{
		Result: xdr.TransactionResultResult{
			InnerResultPair: &xdr.InnerTransactionResultPair{
				TransactionHash: txHash(ledgerSequence),
				Result: xdr.InnerTransactionResult{
					Result: xdr.InnerTransactionResultResult{
						Code:    xdr.TransactionResultCodeTxBadSeq,
						Results: nil,
					},
				},
			},
			Results: &[]xdr.OperationResult{},
		},
	}
}

func txMeta(ledgerSequence uint32) xdr.LedgerCloseMeta {
	txProcessing := []xdr.TransactionResultMetaV2{
		{
			TxApplyProcessing: xdr.TransactionMeta{
				V:          3,
				Operations: &[]xdr.OperationMeta{},
				V3: &xdr.TransactionMetaV3{
					TxResult: transactionResult(ledgerSequence),
				},
			},
			Result: xdr.TransactionResultPairV2{
				TransactionHash: txHash(ledgerSequence),
			},
		},
	}
	return xdr.LedgerCloseMeta{
		V: 2,
		V2: &xdr.LedgerCloseMetaV2{
			LedgerHeader: xdr.LedgerHeaderHistoryEntry{
				Header: xdr.LedgerHeader{
					ScpValue: xdr.StellarValue{
						CloseTime: xdr.TimePoint(ledgerCloseTime(ledgerSequence)),
					},
					LedgerSeq: xdr.Uint32(ledgerSequence),
				},
			},
			TxProcessing: txProcessing,
		},
	}
}

func requirePresent(t *testing.T, store *MemoryStore, ledgerSequence, firstSequence, lastSequence uint32) {
	tx, ok, storeRange := store.GetTransaction(txHash(ledgerSequence))
	require.True(t, ok)
	require.Equal(t, expectedTransaction(ledgerSequence), tx)
	require.Equal(t, expectedStoreRange(firstSequence, lastSequence), storeRange)
}

func TestIngestTransactions(t *testing.T) {
	// Use a small retention window to test eviction
	store, err := NewMemoryStore(3)
	require.NoError(t, err)

	_, ok, storeRange := store.GetTransaction(txHash(1))
	require.False(t, ok)
	require.Equal(t, StoreRange{}, storeRange)

	// Insert ledger 1
	require.NoError(t, store.IngestTransactions(txMeta(1)))
	requirePresent(t, store, 1, 1, 1)

	// Insert ledger 2
	require.NoError(t, store.IngestTransactions(txMeta(2)))
	requirePresent(t, store, 1, 1, 2)
	requirePresent(t, store, 2, 1, 2)

	// Insert ledger 3
	require.NoError(t, store.IngestTransactions(txMeta(3)))
	requirePresent(t, store, 1, 1, 3)
	requirePresent(t, store, 2, 1, 3)
	requirePresent(t, store, 3, 1, 3)

	// Now we have filled the memory store

	// Insert ledger 4, which will cause the window to move and evict ledger 1
	require.NoError(t, store.IngestTransactions(txMeta(4)))
	requirePresent(t, store, 2, 2, 4)
	requirePresent(t, store, 3, 2, 4)
	requirePresent(t, store, 4, 2, 4)

	_, ok, storeRange = store.GetTransaction(txHash(1))
	require.False(t, ok)
	require.Equal(t, expectedStoreRange(2, 4), storeRange)
	require.Equal(t, uint32(3), store.transactionsByLedger.Len())
	require.Len(t, store.transactions, 3)

	// Insert ledger 5, which will cause the window to move and evict ledger 2
	require.NoError(t, store.IngestTransactions(txMeta(5)))
	requirePresent(t, store, 3, 3, 5)
	requirePresent(t, store, 4, 3, 5)
	requirePresent(t, store, 5, 3, 5)

	_, ok, storeRange = store.GetTransaction(txHash(2))
	require.False(t, ok)
	require.Equal(t, expectedStoreRange(3, 5), storeRange)
	require.Equal(t, uint32(3), store.transactionsByLedger.Len())
	require.Len(t, store.transactions, 3)
}
