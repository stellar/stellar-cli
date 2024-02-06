package transactions

import (
	"encoding/hex"
	"fmt"
	"math"
	"runtime"
	"testing"
	"time"

	"github.com/stellar/go/network"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/require"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
)

func expectedTransaction(t *testing.T, ledger uint32, feeBump bool) Transaction {
	tx := Transaction{
		FeeBump:          feeBump,
		ApplicationOrder: 1,
		Ledger:           expectedLedgerInfo(ledger),
	}
	var err error
	tx.Result, err = transactionResult(ledger, feeBump).MarshalBinary()
	require.NoError(t, err)
	tx.Meta, err = xdr.TransactionMeta{
		V:          3,
		Operations: &[]xdr.OperationMeta{},
		V3:         &xdr.TransactionMetaV3{},
	}.MarshalBinary()
	require.NoError(t, err)
	tx.Envelope, err = txEnvelope(ledger, feeBump).MarshalBinary()
	require.NoError(t, err)
	return tx
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
	persistentKey := xdr.ScSymbol("TEMPVAL")
	contractIDBytes, _ := hex.DecodeString("df06d62447fd25da07c0135eed7557e5a5497ee7d15b7fe345bd47e191d8f577")
	var contractID xdr.Hash
	copy(contractID[:], contractIDBytes)
	contractAddress := xdr.ScAddress{
		Type:       xdr.ScAddressTypeScAddressTypeContract,
		ContractId: &contractID,
	}
	xdrTrue := true
	operationChanges := xdr.LedgerEntryChanges{
		{
			Type: xdr.LedgerEntryChangeTypeLedgerEntryState,
			State: &xdr.LedgerEntry{
				LastModifiedLedgerSeq: xdr.Uint32(ledgerSequence - 1),
				Data: xdr.LedgerEntryData{
					Type: xdr.LedgerEntryTypeContractData,
					ContractData: &xdr.ContractDataEntry{
						Contract: contractAddress,
						Key: xdr.ScVal{
							Type: xdr.ScValTypeScvSymbol,
							Sym:  &persistentKey,
						},
						Durability: xdr.ContractDataDurabilityPersistent,
						Val: xdr.ScVal{
							Type: xdr.ScValTypeScvBool,
							B:    &xdrTrue,
						},
					},
				},
			},
		},
		{
			Type: xdr.LedgerEntryChangeTypeLedgerEntryUpdated,
			Updated: &xdr.LedgerEntry{
				LastModifiedLedgerSeq: xdr.Uint32(ledgerSequence - 1),
				Data: xdr.LedgerEntryData{
					Type: xdr.LedgerEntryTypeContractData,
					ContractData: &xdr.ContractDataEntry{
						Contract: xdr.ScAddress{
							Type:       xdr.ScAddressTypeScAddressTypeContract,
							ContractId: &contractID,
						},
						Key: xdr.ScVal{
							Type: xdr.ScValTypeScvSymbol,
							Sym:  &persistentKey,
						},
						Durability: xdr.ContractDataDurabilityPersistent,
						Val: xdr.ScVal{
							Type: xdr.ScValTypeScvBool,
							B:    &xdrTrue,
						},
					},
				},
			},
		},
	}
	txProcessing := []xdr.TransactionResultMeta{
		{
			TxApplyProcessing: xdr.TransactionMeta{
				V: 3,
				Operations: &[]xdr.OperationMeta{
					{
						Changes: operationChanges,
					},
				},
				V3: &xdr.TransactionMetaV3{},
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
		V: 1,
		V1: &xdr.LedgerCloseMetaV1{
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

func txMetaWithEvents(ledgerSequence uint32, feeBump bool) xdr.LedgerCloseMeta {
	tx := txMeta(ledgerSequence, feeBump)
	contractIDBytes, _ := hex.DecodeString("df06d62447fd25da07c0135eed7557e5a5497ee7d15b7fe345bd47e191d8f577")
	var contractID xdr.Hash
	copy(contractID[:], contractIDBytes)
	counter := xdr.ScSymbol("COUNTER")

	tx.V1.TxProcessing[0].TxApplyProcessing.V3 = &xdr.TransactionMetaV3{
		SorobanMeta: &xdr.SorobanTransactionMeta{
			Events: []xdr.ContractEvent{{
				ContractId: &contractID,
				Type:       xdr.ContractEventTypeContract,
				Body: xdr.ContractEventBody{
					V: 0,
					V0: &xdr.ContractEventV0{
						Topics: []xdr.ScVal{{
							Type: xdr.ScValTypeScvSymbol,
							Sym:  &counter,
						}},
						Data: xdr.ScVal{
							Type: xdr.ScValTypeScvSymbol,
							Sym:  &counter,
						},
					},
				},
			}},
			ReturnValue: xdr.ScVal{
				Type: xdr.ScValTypeScvSymbol,
				Sym:  &counter,
			},
		},
	}

	return tx
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
	require.Equal(t, expectedTransaction(t, ledgerSequence, feeBump), tx)
	require.Equal(t, expectedStoreRange(firstSequence, lastSequence), storeRange)
	if feeBump {
		tx, ok, storeRange = store.GetTransaction(txHash(ledgerSequence, true))
		require.True(t, ok)
		require.Equal(t, expectedTransaction(t, ledgerSequence, feeBump), tx)
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

func TestGetTransactionsWithEventData(t *testing.T) {
	store := NewMemoryStore(interfaces.MakeNoOpDeamon(), "passphrase", 100)

	// Insert ledger 1
	metaWithEvents := txMetaWithEvents(1, false)
	require.NoError(t, store.IngestTransactions(metaWithEvents))
	require.Len(t, store.transactions, 1)

	// check events data
	tx, ok, _ := store.GetTransaction(txHash(1, false))
	require.True(t, ok)
	require.NotNil(t, tx.Events)

}

func stableHeapInUse() int64 {
	var (
		m         = runtime.MemStats{}
		prevInUse uint64
		prevNumGC uint32
	)

	for {
		runtime.GC()

		// Sleeping to allow GC to run a few times and collect all temporary data.
		time.Sleep(100 * time.Millisecond)

		runtime.ReadMemStats(&m)

		// Considering heap stable if recent cycle collected less than 10KB.
		if prevNumGC != 0 && m.NumGC > prevNumGC && math.Abs(float64(m.HeapInuse-prevInUse)) < 10*1024 {
			break
		}

		prevInUse = m.HeapInuse
		prevNumGC = m.NumGC
	}

	return int64(m.HeapInuse)
}

func byteCountBinary(b int64) string {
	const unit = 1024
	if b < unit {
		return fmt.Sprintf("%d B", b)
	}
	div, exp := int64(unit), 0
	for n := b / unit; n >= unit; n /= unit {
		div *= unit
		exp++
	}
	return fmt.Sprintf("%.1f %ciB", float64(b)/float64(div), "KMGTPE"[exp])
}

func BenchmarkIngestTransactionsMemory(b *testing.B) {
	roundsNumber := uint32(b.N * 100000)
	// Use a small retention window to test eviction
	store := NewMemoryStore(interfaces.MakeNoOpDeamon(), "passphrase", roundsNumber)

	heapSizeBefore := stableHeapInUse()

	for i := uint32(0); i < roundsNumber; i++ {
		// Insert ledger i
		require.NoError(b, store.IngestTransactions(txMeta(i, false)))
	}
	heapSizeAfter := stableHeapInUse()
	b.ReportMetric(float64(heapSizeAfter), "bytes/100k_transactions")
	b.Logf("Memory consumption for %d transactions %v", roundsNumber, byteCountBinary(heapSizeAfter-heapSizeBefore))

	// we want to generate 500*20000 transactions total, to cover the expected daily amount of transactions.
	projectedTransactionCount := int64(500 * 20000)
	projectedMemoryUtiliztion := (heapSizeAfter - heapSizeBefore) * projectedTransactionCount / int64(roundsNumber)
	b.Logf("Projected memory consumption for %d transactions %v", projectedTransactionCount, byteCountBinary(projectedMemoryUtiliztion))
	b.ReportMetric(float64(projectedMemoryUtiliztion), "bytes/10M_transactions")

	// add another call to store to prevent the GC from collecting.
	store.GetTransaction(xdr.Hash{})
}
