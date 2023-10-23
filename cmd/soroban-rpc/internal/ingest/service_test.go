package ingest

import (
	"context"
	"encoding/hex"
	"errors"
	"sync"
	"testing"
	"time"

	"github.com/stellar/go/ingest/ledgerbackend"
	"github.com/stellar/go/network"
	supportlog "github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/daemon/interfaces"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/events"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/transactions"
)

type ErrorReadWriter struct {
}

func (rw *ErrorReadWriter) GetLatestLedgerSequence(ctx context.Context) (uint32, error) {
	return 0, errors.New("could not get latest ledger sequence")
}
func (rw *ErrorReadWriter) NewTx(ctx context.Context) (db.WriteTx, error) {
	return nil, errors.New("could not create new tx")
}

func TestRetryRunningIngestion(t *testing.T) {

	var retryWg sync.WaitGroup
	retryWg.Add(1)

	numRetries := 0
	var lastErr error
	incrementRetry := func(err error, dur time.Duration) {
		defer retryWg.Done()
		numRetries++
		lastErr = err
	}
	config := Config{
		Logger:            supportlog.New(),
		DB:                &ErrorReadWriter{},
		EventStore:        nil,
		TransactionStore:  nil,
		NetworkPassPhrase: "",
		Archive:           nil,
		LedgerBackend:     nil,
		Timeout:           time.Second,
		OnIngestionRetry:  incrementRetry,
		Daemon:            interfaces.MakeNoOpDeamon(),
	}
	service := NewService(config)
	retryWg.Wait()
	service.Close()
	assert.Equal(t, 1, numRetries)
	assert.Error(t, lastErr)
	assert.ErrorContains(t, lastErr, "could not get latest ledger sequence")
}

func TestIngestion(t *testing.T) {
	mockDB := &MockDB{}
	mockLedgerBackend := &ledgerbackend.MockDatabaseBackend{}
	daemon := interfaces.MakeNoOpDeamon()
	config := Config{
		Logger:            supportlog.New(),
		DB:                mockDB,
		EventStore:        events.NewMemoryStore(daemon, network.TestNetworkPassphrase, 1),
		TransactionStore:  transactions.NewMemoryStore(daemon, network.TestNetworkPassphrase, 1),
		LedgerBackend:     mockLedgerBackend,
		Daemon:            daemon,
		NetworkPassPhrase: network.TestNetworkPassphrase,
	}
	sequence := uint32(3)
	service := newService(config)
	mockTx := &MockTx{}
	mockLedgerEntryWriter := &MockLedgerEntryWriter{}
	mockLedgerWriter := &MockLedgerWriter{}
	ctx := context.Background()
	mockDB.On("NewTx", ctx).Return(mockTx, nil).Once()
	mockTx.On("Commit", sequence).Return(nil).Once()
	mockTx.On("Rollback").Return(nil).Once()
	mockTx.On("LedgerEntryWriter").Return(mockLedgerEntryWriter).Twice()
	mockTx.On("LedgerWriter").Return(mockLedgerWriter).Once()

	src := xdr.MustAddress("GBXGQJWVLWOYHFLVTKWV5FGHA3LNYY2JQKM7OAJAUEQFU6LPCSEFVXON")
	firstTx := xdr.TransactionEnvelope{
		Type: xdr.EnvelopeTypeEnvelopeTypeTx,
		V1: &xdr.TransactionV1Envelope{
			Tx: xdr.Transaction{
				Fee:           1,
				SourceAccount: src.ToMuxedAccount(),
			},
		},
	}
	firstTxHash, err := network.HashTransactionInEnvelope(firstTx, network.TestNetworkPassphrase)
	assert.NoError(t, err)

	baseFee := xdr.Int64(100)
	tempKey := xdr.ScSymbol("TEMPKEY")
	persistentKey := xdr.ScSymbol("TEMPVAL")
	contractIDBytes, err := hex.DecodeString("df06d62447fd25da07c0135eed7557e5a5497ee7d15b7fe345bd47e191d8f577")
	assert.NoError(t, err)
	var contractID xdr.Hash
	copy(contractID[:], contractIDBytes)
	contractAddress := xdr.ScAddress{
		Type:       xdr.ScAddressTypeScAddressTypeContract,
		ContractId: &contractID,
	}
	operationChanges := xdr.LedgerEntryChanges{
		{
			Type: xdr.LedgerEntryChangeTypeLedgerEntryState,
			State: &xdr.LedgerEntry{
				LastModifiedLedgerSeq: 1,
				Data: xdr.LedgerEntryData{
					Type: xdr.LedgerEntryTypeContractData,
					ContractData: &xdr.ContractDataEntry{
						Contract: contractAddress,
						Key: xdr.ScVal{
							Type: xdr.ScValTypeScvSymbol,
							Sym:  &persistentKey,
						},
						Durability: xdr.ContractDataDurabilityPersistent,
					},
				},
			},
		},
		{
			Type: xdr.LedgerEntryChangeTypeLedgerEntryUpdated,
			Updated: &xdr.LedgerEntry{
				LastModifiedLedgerSeq: 1,
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
					},
				},
			},
		},
	}
	evictedPersistentLedgerEntry := xdr.LedgerEntry{
		LastModifiedLedgerSeq: 123,
		Data: xdr.LedgerEntryData{
			Type: xdr.LedgerEntryTypeContractData,
			ContractData: &xdr.ContractDataEntry{
				Contract: contractAddress,
				Key: xdr.ScVal{
					Type: xdr.ScValTypeScvSymbol,
					Sym:  &persistentKey,
				},
				Durability: xdr.ContractDataDurabilityTemporary,
			},
		},
	}
	evictedTempLedgerKey := xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeContractData,
		ContractData: &xdr.LedgerKeyContractData{
			Contract: contractAddress,
			Key: xdr.ScVal{
				Type: xdr.ScValTypeScvSymbol,
				Sym:  &tempKey,
			},
			Durability: xdr.ContractDataDurabilityTemporary,
		},
	}
	ledger := xdr.LedgerCloseMeta{
		V: 1,
		V1: &xdr.LedgerCloseMetaV1{
			LedgerHeader: xdr.LedgerHeaderHistoryEntry{Header: xdr.LedgerHeader{LedgerVersion: 10}},
			TxSet: xdr.GeneralizedTransactionSet{
				V: 1,
				V1TxSet: &xdr.TransactionSetV1{
					PreviousLedgerHash: xdr.Hash{1, 2, 3},
					Phases: []xdr.TransactionPhase{
						{
							V0Components: &[]xdr.TxSetComponent{
								{
									Type: xdr.TxSetComponentTypeTxsetCompTxsMaybeDiscountedFee,
									TxsMaybeDiscountedFee: &xdr.TxSetComponentTxsMaybeDiscountedFee{
										BaseFee: &baseFee,
										Txs: []xdr.TransactionEnvelope{
											firstTx,
										},
									},
								},
							},
						},
					},
				},
			},
			TxProcessing: []xdr.TransactionResultMeta{
				{
					Result:        xdr.TransactionResultPair{TransactionHash: firstTxHash},
					FeeProcessing: xdr.LedgerEntryChanges{},
					TxApplyProcessing: xdr.TransactionMeta{
						V: 3,
						V3: &xdr.TransactionMetaV3{
							Operations: []xdr.OperationMeta{
								{
									Changes: operationChanges,
								},
							},
						},
					},
				},
			},
			UpgradesProcessing:             []xdr.UpgradeEntryMeta{},
			EvictedTemporaryLedgerKeys:     []xdr.LedgerKey{evictedTempLedgerKey},
			EvictedPersistentLedgerEntries: []xdr.LedgerEntry{evictedPersistentLedgerEntry},
		},
	}
	mockLedgerBackend.On("GetLedger", ctx, sequence).
		Return(ledger, nil).Once()
	mockLedgerEntryWriter.On("UpsertLedgerEntry", operationChanges[1].MustUpdated()).
		Return(nil).Once()
	evictedPresistentLedgerKey, err := evictedPersistentLedgerEntry.LedgerKey()
	assert.NoError(t, err)
	mockLedgerEntryWriter.On("DeleteLedgerEntry", evictedPresistentLedgerKey).
		Return(nil).Once()
	mockLedgerEntryWriter.On("DeleteLedgerEntry", evictedTempLedgerKey).
		Return(nil).Once()
	mockLedgerWriter.On("InsertLedger", ledger).
		Return(nil).Once()
	assert.NoError(t, service.ingest(ctx, sequence))

	mockDB.AssertExpectations(t)
	mockTx.AssertExpectations(t)
	mockLedgerEntryWriter.AssertExpectations(t)
	mockLedgerWriter.AssertExpectations(t)
	mockLedgerBackend.AssertExpectations(t)
}
