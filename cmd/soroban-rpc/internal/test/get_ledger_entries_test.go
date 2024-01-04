package test

import (
	"context"
	"crypto/sha256"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/stellar/go/keypair"
	"github.com/stellar/go/txnbuild"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

func TestGetLedgerEntriesNotFound(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	contractID := getContractID(t, sourceAccount, testSalt, StandaloneNetworkPassphrase)
	contractIDHash := xdr.Hash(contractID)
	keyB64, err := xdr.MarshalBase64(xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeContractData,
		ContractData: &xdr.LedgerKeyContractData{
			Contract: xdr.ScAddress{
				Type:       xdr.ScAddressTypeScAddressTypeContract,
				ContractId: &contractIDHash,
			},
			Key: xdr.ScVal{
				Type: xdr.ScValTypeScvLedgerKeyContractInstance,
			},
			Durability: xdr.ContractDataDurabilityPersistent,
		},
	})
	require.NoError(t, err)

	var keys []string
	keys = append(keys, keyB64)
	request := methods.GetLedgerEntriesRequest{
		Keys: keys,
	}

	var result methods.GetLedgerEntriesResponse
	err = client.CallResult(context.Background(), "getLedgerEntries", request, &result)
	require.NoError(t, err)

	assert.Equal(t, 0, len(result.Entries))
	assert.Greater(t, result.LatestLedger, uint32(0))
}

func TestGetLedgerEntriesInvalidParams(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	var keys []string
	keys = append(keys, "<>@@#$")
	request := methods.GetLedgerEntriesRequest{
		Keys: keys,
	}

	var result methods.GetLedgerEntriesResponse
	jsonRPCErr := client.CallResult(context.Background(), "getLedgerEntries", request, &result).(*jrpc2.Error)
	assert.Contains(t, jsonRPCErr.Message, "cannot unmarshal key value")
	assert.Equal(t, jrpc2.InvalidParams, jsonRPCErr.Code)
}

func TestGetLedgerEntriesSucceeds(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase)
	address := sourceAccount.Address()
	account := txnbuild.NewSimpleAccount(address, 0)

	contractBinary := getHelloWorldContract(t)
	params := preflightTransactionParams(t, client, txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createInstallContractCodeOperation(account.AccountID, contractBinary),
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	tx, err := txnbuild.NewTransaction(params)
	assert.NoError(t, err)
	sendSuccessfulTransaction(t, client, sourceAccount, tx)

	params = preflightTransactionParams(t, client, txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createCreateContractOperation(address, contractBinary),
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	tx, err = txnbuild.NewTransaction(params)
	assert.NoError(t, err)
	sendSuccessfulTransaction(t, client, sourceAccount, tx)

	contractID := getContractID(t, address, testSalt, StandaloneNetworkPassphrase)

	contractHash := sha256.Sum256(contractBinary)
	contractCodeKeyB64, err := xdr.MarshalBase64(xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeContractCode,
		ContractCode: &xdr.LedgerKeyContractCode{
			Hash: contractHash,
		},
	})

	// Doesn't exist.
	notFoundKeyB64, err := xdr.MarshalBase64(getCounterLedgerKey(contractID))
	require.NoError(t, err)

	contractIDHash := xdr.Hash(contractID)
	contractInstanceKeyB64, err := xdr.MarshalBase64(xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeContractData,
		ContractData: &xdr.LedgerKeyContractData{
			Contract: xdr.ScAddress{
				Type:       xdr.ScAddressTypeScAddressTypeContract,
				ContractId: &contractIDHash,
			},
			Key: xdr.ScVal{
				Type: xdr.ScValTypeScvLedgerKeyContractInstance,
			},
			Durability: xdr.ContractDataDurabilityPersistent,
		},
	})
	require.NoError(t, err)

	keys := []string{contractCodeKeyB64, notFoundKeyB64, contractInstanceKeyB64}
	request := methods.GetLedgerEntriesRequest{
		Keys: keys,
	}

	var result methods.GetLedgerEntriesResponse
	err = client.CallResult(context.Background(), "getLedgerEntries", request, &result)
	require.NoError(t, err)
	require.Equal(t, 2, len(result.Entries))
	require.Greater(t, result.LatestLedger, uint32(0))

	require.Greater(t, result.Entries[0].LastModifiedLedger, uint32(0))
	require.LessOrEqual(t, result.Entries[0].LastModifiedLedger, result.LatestLedger)
	require.NotNil(t, result.Entries[0].LiveUntilLedgerSeq)
	require.Greater(t, *result.Entries[0].LiveUntilLedgerSeq, result.LatestLedger)
	require.Equal(t, contractCodeKeyB64, result.Entries[0].Key)
	var firstEntry xdr.LedgerEntryData
	require.NoError(t, xdr.SafeUnmarshalBase64(result.Entries[0].XDR, &firstEntry))
	require.Equal(t, xdr.LedgerEntryTypeContractCode, firstEntry.Type)
	require.Equal(t, contractBinary, firstEntry.MustContractCode().Code)

	require.Greater(t, result.Entries[1].LastModifiedLedger, uint32(0))
	require.LessOrEqual(t, result.Entries[1].LastModifiedLedger, result.LatestLedger)
	require.NotNil(t, result.Entries[1].LiveUntilLedgerSeq)
	require.Greater(t, *result.Entries[1].LiveUntilLedgerSeq, result.LatestLedger)
	require.Equal(t, contractInstanceKeyB64, result.Entries[1].Key)
	var secondEntry xdr.LedgerEntryData
	require.NoError(t, xdr.SafeUnmarshalBase64(result.Entries[1].XDR, &secondEntry))
	require.Equal(t, xdr.LedgerEntryTypeContractData, secondEntry.Type)
	require.True(t, secondEntry.MustContractData().Key.Equals(xdr.ScVal{
		Type: xdr.ScValTypeScvLedgerKeyContractInstance,
	}))
}
