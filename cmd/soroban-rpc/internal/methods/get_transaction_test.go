package methods

import (
	"encoding/hex"
	"testing"

	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"

	"github.com/stellar/go/network"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

func txHash(acctSeq uint32) xdr.Hash {
	envelope := txEnvelope(acctSeq)
	hash, err := network.HashTransactionInEnvelope(envelope, "passphrase")
	if err != nil {
		panic(err)
	}

	return hash
}

func ledgerCloseTime(ledgerSequence uint32) int64 {
	return int64(ledgerSequence)*25 + 100
}

func transactionResult(successful bool) xdr.TransactionResult {
	code := xdr.TransactionResultCodeTxBadSeq
	if successful {
		code = xdr.TransactionResultCodeTxSuccess
	}
	opResults := []xdr.OperationResult{}
	return xdr.TransactionResult{
		FeeCharged: 100,
		Result: xdr.TransactionResultResult{
			Code:    code,
			Results: &opResults,
		},
	}
}

func txMeta(acctSeq uint32, successful bool) xdr.LedgerCloseMeta {
	envelope := txEnvelope(acctSeq)

	txProcessing := []xdr.TransactionResultMeta{
		{
			TxApplyProcessing: xdr.TransactionMeta{
				V:          3,
				Operations: &[]xdr.OperationMeta{},
				V3:         &xdr.TransactionMetaV3{},
			},
			Result: xdr.TransactionResultPair{
				TransactionHash: txHash(acctSeq),
				Result:          transactionResult(successful),
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
						CloseTime: xdr.TimePoint(ledgerCloseTime(acctSeq + 100)),
					},
					LedgerSeq: xdr.Uint32(acctSeq + 100),
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

func txEnvelope(acctSeq uint32) xdr.TransactionEnvelope {
	envelope, err := xdr.NewTransactionEnvelope(xdr.EnvelopeTypeEnvelopeTypeTx, xdr.TransactionV1Envelope{
		Tx: xdr.Transaction{
			Fee:           1,
			SeqNum:        xdr.SequenceNumber(acctSeq),
			SourceAccount: xdr.MustMuxedAddress("MA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVAAAAAAAAAAAAAJLK"),
		},
	})
	if err != nil {
		panic(err)
	}
	return envelope
}

func TestGetTransaction(t *testing.T) {
	store := transactions.NewMemoryStore(interfaces.MakeNoOpDeamon(), "passphrase", 100)
	_, err := GetTransaction(store, GetTransactionRequest{"ab"})
	assert.EqualError(t, err, "[-32602] unexpected hash length (2)")
	_, err = GetTransaction(store, GetTransactionRequest{"foo                                                              "})
	assert.EqualError(t, err, "[-32602] incorrect hash: encoding/hex: invalid byte: U+006F 'o'")

	hash := "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	tx, err := GetTransaction(store, GetTransactionRequest{hash})
	assert.NoError(t, err)
	assert.Equal(t, GetTransactionResponse{
		Status: TransactionStatusNotFound,
	}, tx)

	meta := txMeta(1, true)
	err = store.IngestTransactions(meta)
	assert.NoError(t, err)

	xdrHash := txHash(1)
	hash = hex.EncodeToString(xdrHash[:])
	tx, err = GetTransaction(store, GetTransactionRequest{hash})
	assert.NoError(t, err)

	expectedTxResult, err := xdr.MarshalBase64(meta.V2.TxProcessing[0].Result.Result)
	assert.NoError(t, err)
	expectedEnvelope, err := xdr.MarshalBase64(txEnvelope(1))
	assert.NoError(t, err)
	expectedTxMeta, err := xdr.MarshalBase64(meta.V2.TxProcessing[0].TxApplyProcessing)
	assert.NoError(t, err)
	assert.Equal(t, GetTransactionResponse{
		Status:                TransactionStatusSuccess,
		LatestLedger:          101,
		LatestLedgerCloseTime: 2625,
		OldestLedger:          101,
		OldestLedgerCloseTime: 2625,
		ApplicationOrder:      1,
		FeeBump:               false,
		EnvelopeXdr:           expectedEnvelope,
		ResultXdr:             expectedTxResult,
		ResultMetaXdr:         expectedTxMeta,
		Ledger:                101,
		LedgerCloseTime:       2625,
	}, tx)

	// ingest another (failed) transaction
	meta = txMeta(2, false)
	err = store.IngestTransactions(meta)
	assert.NoError(t, err)

	// the first transaction should still be there
	tx, err = GetTransaction(store, GetTransactionRequest{hash})
	assert.NoError(t, err)
	assert.Equal(t, GetTransactionResponse{
		Status:                TransactionStatusSuccess,
		LatestLedger:          102,
		LatestLedgerCloseTime: 2650,
		OldestLedger:          101,
		OldestLedgerCloseTime: 2625,
		ApplicationOrder:      1,
		FeeBump:               false,
		EnvelopeXdr:           expectedEnvelope,
		ResultXdr:             expectedTxResult,
		ResultMetaXdr:         expectedTxMeta,
		Ledger:                101,
		LedgerCloseTime:       2625,
	}, tx)

	// the new transaction should also be there
	xdrHash = txHash(2)
	hash = hex.EncodeToString(xdrHash[:])

	expectedTxResult, err = xdr.MarshalBase64(meta.V2.TxProcessing[0].Result.Result)
	assert.NoError(t, err)
	expectedEnvelope, err = xdr.MarshalBase64(txEnvelope(2))
	assert.NoError(t, err)
	expectedTxMeta, err = xdr.MarshalBase64(meta.V2.TxProcessing[0].TxApplyProcessing)
	assert.NoError(t, err)

	tx, err = GetTransaction(store, GetTransactionRequest{hash})
	assert.NoError(t, err)
	assert.NoError(t, err)
	assert.Equal(t, GetTransactionResponse{
		Status:                TransactionStatusFailed,
		LatestLedger:          102,
		LatestLedgerCloseTime: 2650,
		OldestLedger:          101,
		OldestLedgerCloseTime: 2625,
		ApplicationOrder:      1,
		FeeBump:               false,
		EnvelopeXdr:           expectedEnvelope,
		ResultXdr:             expectedTxResult,
		ResultMetaXdr:         expectedTxMeta,
		Ledger:                102,
		LedgerCloseTime:       2650,
	}, tx)
}
