package transactions

import (
	"testing"

	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/require"
)

func expectedTransaction(ledger uint32, feeBump bool) Transaction {
	return Transaction{
		Result:           transactionResult(ledger, feeBump),
		ApplicationOrder: 1,
		FeeBump:          feeBump,
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

func innerTxHash(ledgerSequence uint32) xdr.Hash {
	return txHash(ledgerSequence * 1000)
}

func ledgerCloseTime(ledgerSequence uint32) int64 {
	return int64(ledgerSequence)*25 + 100
}

func transactionResult(ledgerSequence uint32, feeBump bool) xdr.TransactionResult {
	if feeBump {
		return xdr.TransactionResult{
			FeeCharged: 100,
			Result: xdr.TransactionResultResult{
				Code: xdr.TransactionResultCodeTxFeeBumpInnerFailed,
				InnerResultPair: &xdr.InnerTransactionResultPair{
					TransactionHash: innerTxHash(ledgerSequence),
					Result: xdr.InnerTransactionResult{
						Result: xdr.InnerTransactionResultResult{
							Code: xdr.TransactionResultCodeTxBadSeq,
						},
					},
				},
			},
		}
	}
	return xdr.TransactionResult{
		FeeCharged: 100,
		Result: xdr.TransactionResultResult{
			Code: xdr.TransactionResultCodeTxBadSeq,
		},
	}
}

func txMeta(ledgerSequence uint32, feeBump bool) xdr.LedgerCloseMeta {
	txProcessing := []xdr.TransactionResultMetaV2{
		{
			TxApplyProcessing: xdr.TransactionMeta{
				V:          3,
				Operations: &[]xdr.OperationMeta{},
				V3: &xdr.TransactionMetaV3{
					TxResult: transactionResult(ledgerSequence, feeBump),
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

func requirePresent(t *testing.T, store *MemoryStore, feeBump bool, ledgerSequence, firstSequence, lastSequence uint32) {
	tx, ok, storeRange := store.GetTransaction(txHash(ledgerSequence))
	require.True(t, ok)
	require.Equal(t, expectedTransaction(ledgerSequence, feeBump), tx)
	require.Equal(t, expectedStoreRange(firstSequence, lastSequence), storeRange)
	if feeBump {
		tx, ok, storeRange = store.GetTransaction(innerTxHash(ledgerSequence))
		require.True(t, ok)
		require.Equal(t, expectedTransaction(ledgerSequence, feeBump), tx)
		require.Equal(t, expectedStoreRange(firstSequence, lastSequence), storeRange)
	}
}

func TestIngestTransactions(t *testing.T) {
	// Use a small retention window to test eviction
	store, err := NewMemoryStore(3)
	require.NoError(t, err)

	_, ok, storeRange := store.GetTransaction(txHash(1))
	require.False(t, ok)
	require.Equal(t, StoreRange{}, storeRange)

	// Insert ledger 1
	require.NoError(t, store.IngestTransactions(txMeta(1, false)))
	requirePresent(t, store, false, 1, 1, 1)
	require.Len(t, store.transactions, 1)

	// Insert ledger 2
	require.NoError(t, store.IngestTransactions(txMeta(2, true)))
	requirePresent(t, store, false, 1, 1, 2)
	requirePresent(t, store, true, 2, 1, 2)
	require.Len(t, store.transactions, 3)

	// Insert ledger 3
	require.NoError(t, store.IngestTransactions(txMeta(3, false)))
	requirePresent(t, store, false, 1, 1, 3)
	requirePresent(t, store, true, 2, 1, 3)
	requirePresent(t, store, false, 3, 1, 3)
	require.Len(t, store.transactions, 4)

	// Now we have filled the memory store

	// Insert ledger 4, which will cause the window to move and evict ledger 1
	require.NoError(t, store.IngestTransactions(txMeta(4, false)))
	requirePresent(t, store, true, 2, 2, 4)
	requirePresent(t, store, false, 3, 2, 4)
	requirePresent(t, store, false, 4, 2, 4)

	_, ok, storeRange = store.GetTransaction(txHash(1))
	require.False(t, ok)
	require.Equal(t, expectedStoreRange(2, 4), storeRange)
	require.Equal(t, uint32(3), store.transactionsByLedger.Len())
	require.Len(t, store.transactions, 4)

	// Insert ledger 5, which will cause the window to move and evict ledger 2
	require.NoError(t, store.IngestTransactions(txMeta(5, false)))
	requirePresent(t, store, false, 3, 3, 5)
	requirePresent(t, store, false, 4, 3, 5)
	requirePresent(t, store, false, 5, 3, 5)

	_, ok, storeRange = store.GetTransaction(txHash(2))
	require.False(t, ok)
	require.Equal(t, expectedStoreRange(3, 5), storeRange)
	require.Equal(t, uint32(3), store.transactionsByLedger.Len())
	require.Len(t, store.transactions, 3)

	_, ok, storeRange = store.GetTransaction(innerTxHash(2))
	require.False(t, ok)
	require.Equal(t, expectedStoreRange(3, 5), storeRange)
	require.Equal(t, uint32(3), store.transactionsByLedger.Len())
	require.Len(t, store.transactions, 3)
}
