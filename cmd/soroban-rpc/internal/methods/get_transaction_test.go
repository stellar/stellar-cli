package methods

import (
	"encoding/hex"
	"testing"

	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

func txMeta(ledgerSequence uint32, closeTime int64, hash xdr.Hash, sucessful bool) xdr.LedgerCloseMeta {
	code := xdr.TransactionResultCodeTxSuccess
	if !sucessful {
		code = xdr.TransactionResultCodeTxBadSeq
	}
	txProcessing := []xdr.TransactionResultMetaV2{
		{
			TxApplyProcessing: xdr.TransactionMeta{
				V:          3,
				Operations: &[]xdr.OperationMeta{},
				V3: &xdr.TransactionMetaV3{
					TxResult: xdr.TransactionResult{
						Result: xdr.TransactionResultResult{
							Code: code,
							InnerResultPair: &xdr.InnerTransactionResultPair{
								TransactionHash: hash,
								Result: xdr.InnerTransactionResult{
									Result: xdr.InnerTransactionResultResult{
										Code:    code,
										Results: nil,
									},
								},
							},
							Results: &[]xdr.OperationResult{},
						},
					},
				},
			},
			Result: xdr.TransactionResultPairV2{
				TransactionHash: hash,
			},
		},
	}
	return xdr.LedgerCloseMeta{
		V: 2,
		V2: &xdr.LedgerCloseMetaV2{
			LedgerHeader: xdr.LedgerHeaderHistoryEntry{
				Header: xdr.LedgerHeader{
					ScpValue: xdr.StellarValue{
						CloseTime: xdr.TimePoint(closeTime),
					},
					LedgerSeq: xdr.Uint32(ledgerSequence),
				},
			},
			TxProcessing: txProcessing,
		},
	}
}

func TestGetTransaction(t *testing.T) {
	store, err := transactions.NewMemoryStore(100)
	assert.NoError(t, err)
	_, err = GetTransaction(store, GetTransactionRequest{"ab"})
	assert.EqualError(t, err, "[-32602] unexpected hash length (2)")
	_, err = GetTransaction(store, GetTransactionRequest{"foo                                                              "})
	assert.EqualError(t, err, "[-32602] incorrect hash: encoding/hex: invalid byte: U+006F 'o'")

	hash := "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	tx, err := GetTransaction(store, GetTransactionRequest{hash})
	assert.NoError(t, err)
	assert.Equal(t, GetTransactionResponse{
		Status: TransactionStatusNotFound,
	}, tx)

	var xdrHash xdr.Hash
	hex.Decode(xdrHash[:], []byte(hash))
	meta := txMeta(1, 250, xdrHash, true)
	err = store.IngestTransactions(meta)
	assert.NoError(t, err)

	tx, err = GetTransaction(store, GetTransactionRequest{hash})
	assert.NoError(t, err)

	expectedTxResult, err := xdr.MarshalBase64(meta.V2.TxProcessing[0].TxApplyProcessing.V3.TxResult)
	assert.NoError(t, err)
	assert.Equal(t, GetTransactionResponse{
		Status:                TransactionStatusSuccess,
		LatestLedger:          1,
		LatestLedgerCloseTime: 250,
		OldestLedger:          1,
		OldestLedgerCloseTime: 250,
		ApplicationOrder:      1,
		ResultXdr:             expectedTxResult,
		Ledger:                1,
		LedgerCloseTime:       250,
	}, tx)

	// ingest another (failed) transaction
	hash2 := "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
	hex.Decode(xdrHash[:], []byte(hash2))
	meta2 := txMeta(2, 350, xdrHash, false)
	err = store.IngestTransactions(meta2)

	// the first transaction should still be there
	tx, err = GetTransaction(store, GetTransactionRequest{hash})
	assert.NoError(t, err)
	assert.NoError(t, err)
	assert.Equal(t, GetTransactionResponse{
		Status:                TransactionStatusSuccess,
		LatestLedger:          2,
		LatestLedgerCloseTime: 350,
		OldestLedger:          1,
		OldestLedgerCloseTime: 250,
		ApplicationOrder:      1,
		ResultXdr:             expectedTxResult,
		Ledger:                1,
		LedgerCloseTime:       250,
	}, tx)

	// the new transaction should also be there
	expectedTxResult, err = xdr.MarshalBase64(meta2.V2.TxProcessing[0].TxApplyProcessing.V3.TxResult)
	assert.NoError(t, err)

	tx, err = GetTransaction(store, GetTransactionRequest{hash2})
	assert.NoError(t, err)
	assert.NoError(t, err)
	assert.Equal(t, GetTransactionResponse{
		Status:                TransactionStatusFailed,
		LatestLedger:          2,
		LatestLedgerCloseTime: 350,
		OldestLedger:          1,
		OldestLedgerCloseTime: 250,
		ApplicationOrder:      1,
		ResultXdr:             expectedTxResult,
		Ledger:                2,
		LedgerCloseTime:       350,
	}, tx)
}
