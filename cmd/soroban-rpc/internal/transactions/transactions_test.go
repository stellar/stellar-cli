package transactions

import (
	"testing"

	"github.com/stellar/go/network"
	"github.com/stellar/go/xdr"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
	"github.com/stretchr/testify/require"
)

func expectedTransaction(ledger uint32, feeBump bool) Transaction {
	return Transaction{
		Result: transactionResult(ledger, feeBump),
		Meta: xdr.TransactionMeta{
			V:          3,
			Operations: &[]xdr.OperationMeta{},
			V3:         &xdr.TransactionMetaV3{},
		},
		Envelope:         txEnvelope(ledger, feeBump),
		FeeBump:          feeBump,
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

func txHash(ledgerSequence uint32, feebump bool) xdr.Hash {
	envelope := txEnvelope(ledgerSequence, feebump)
	hash, err := network.HashTransactionInEnvelope(envelope, "passphrase")
	if err != nil {
		panic(err)
	}

	return hash
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
					TransactionHash: txHash(ledgerSequence, false),
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
	envelope := txEnvelope(ledgerSequence, feeBump)

	txProcessing := []xdr.TransactionResultMeta{
		{
			TxApplyProcessing: xdr.TransactionMeta{
				V:          3,
				Operations: &[]xdr.OperationMeta{},
				V3:         &xdr.TransactionMetaV3{},
			},
			Result: xdr.TransactionResultPair{
				TransactionHash: txHash(ledgerSequence, feeBump),
				Result:          transactionResult(ledgerSequence, feeBump),
			},
		},
	}

	components := []xdr.TxSetComponent{
		{
			Type: xdr.TxSetComponentTypeTxsetCompTxsMaybeDiscountedFee,
			TxsMaybeDiscountedFee: &xdr.TxSetComponentTxsMaybeDiscountedFee{
				BaseFee: nil,
				Txs: []xdr.TransactionEnvelope{
					envelope,
				},
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
			TxSet: xdr.GeneralizedTransactionSet{
				V: 1,
				V1TxSet: &xdr.TransactionSetV1{
					PreviousLedgerHash: xdr.Hash{1},
					Phases: []xdr.TransactionPhase{
						{
							V:            0,
							V0Components: &components,
						},
					},
				},
			},
		},
	}
}

func txEnvelope(ledgerSequence uint32, feeBump bool) xdr.TransactionEnvelope {
	var envelope xdr.TransactionEnvelope
	var err error
	if feeBump {
		envelope, err = xdr.NewTransactionEnvelope(xdr.EnvelopeTypeEnvelopeTypeTxFeeBump, xdr.FeeBumpTransactionEnvelope{
			Tx: xdr.FeeBumpTransaction{
				Fee:       10,
				FeeSource: xdr.MustMuxedAddress("MA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVAAAAAAAAAAAAAJLK"),
				InnerTx: xdr.FeeBumpTransactionInnerTx{
					Type: xdr.EnvelopeTypeEnvelopeTypeTx,
					V1: &xdr.TransactionV1Envelope{
						Tx: xdr.Transaction{
							Fee:           1,
							SeqNum:        xdr.SequenceNumber(ledgerSequence + 90),
							SourceAccount: xdr.MustMuxedAddress("MA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVAAAAAAAAAAAAAJLK"),
						},
					},
				},
			},
		})
	} else {
		envelope, err = xdr.NewTransactionEnvelope(xdr.EnvelopeTypeEnvelopeTypeTx, xdr.TransactionV1Envelope{
			Tx: xdr.Transaction{
				Fee:           1,
				SeqNum:        xdr.SequenceNumber(ledgerSequence + 90),
				SourceAccount: xdr.MustMuxedAddress("MA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVAAAAAAAAAAAAAJLK"),
			},
		})
	}
	if err != nil {
		panic(err)
	}
	return envelope
}

func requirePresent(t *testing.T, store *MemoryStore, feeBump bool, ledgerSequence, firstSequence, lastSequence uint32) {
	tx, ok, storeRange := store.GetTransaction(txHash(ledgerSequence, false))
	require.True(t, ok)
	require.Equal(t, expectedTransaction(ledgerSequence, feeBump), tx)
	require.Equal(t, expectedStoreRange(firstSequence, lastSequence), storeRange)
	if feeBump {
		tx, ok, storeRange = store.GetTransaction(txHash(ledgerSequence, true))
		require.True(t, ok)
		require.Equal(t, expectedTransaction(ledgerSequence, feeBump), tx)
		require.Equal(t, expectedStoreRange(firstSequence, lastSequence), storeRange)
	}
}

func TestIngestTransactions(t *testing.T) {
	// Use a small retention window to test eviction
	store := NewMemoryStore(interfaces.MakeNoOpDeamon(), "passphrase", 3)

	_, ok, storeRange := store.GetTransaction(txHash(1, false))
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

	_, ok, storeRange = store.GetTransaction(txHash(1, false))
	require.False(t, ok)
	require.Equal(t, expectedStoreRange(2, 4), storeRange)
	require.Equal(t, uint32(3), store.transactionsByLedger.Len())
	require.Len(t, store.transactions, 4)

	// Insert ledger 5, which will cause the window to move and evict ledger 2
	require.NoError(t, store.IngestTransactions(txMeta(5, false)))
	requirePresent(t, store, false, 3, 3, 5)
	requirePresent(t, store, false, 4, 3, 5)
	requirePresent(t, store, false, 5, 3, 5)

	_, ok, storeRange = store.GetTransaction(txHash(2, false))
	require.False(t, ok)
	require.Equal(t, expectedStoreRange(3, 5), storeRange)
	require.Equal(t, uint32(3), store.transactionsByLedger.Len())
	require.Len(t, store.transactions, 3)

	_, ok, storeRange = store.GetTransaction(txHash(2, true))
	require.False(t, ok)
	require.Equal(t, expectedStoreRange(3, 5), storeRange)
	require.Equal(t, uint32(3), store.transactionsByLedger.Len())
	require.Len(t, store.transactions, 3)
}
