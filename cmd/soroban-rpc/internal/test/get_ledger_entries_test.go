package test

import (
	"context"
	"crypto/sha256"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/stellar/go/keypair"
	proto "github.com/stellar/go/protocols/stellarcore"
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
			BodyType:   xdr.ContractEntryBodyTypeDataEntry,
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
	assert.Greater(t, result.LatestLedger, int64(0))
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
	assert.Equal(t, code.InvalidParams, jsonRPCErr.Code)
}

func TestGetLedgerEntriesSucceeds(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	kp := keypair.Root(StandaloneNetworkPassphrase)
	account := txnbuild.NewSimpleAccount(kp.Address(), 0)

	params := preflightTransactionParams(t, client, txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createInstallContractCodeOperation(account.AccountID, testContract),
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	tx, err := txnbuild.NewTransaction(params)
	require.NoError(t, err)
	tx, err = tx.Sign(StandaloneNetworkPassphrase, kp)
	require.NoError(t, err)
	b64, err := tx.Base64()
	require.NoError(t, err)

	sendTxRequest := methods.SendTransactionRequest{Transaction: b64}
	var sendTxResponse methods.SendTransactionResponse
	err = client.CallResult(context.Background(), "sendTransaction", sendTxRequest, &sendTxResponse)
	assert.NoError(t, err)
	assert.Equal(t, proto.TXStatusPending, sendTxResponse.Status)

	txStatusResponse := getTransaction(t, client, sendTxResponse.Hash)
	assert.Equal(t, methods.TransactionStatusSuccess, txStatusResponse.Status)

	contractHash := sha256.Sum256(testContract)
	contractKeyB64, err := xdr.MarshalBase64(xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeContractCode,
		ContractCode: &xdr.LedgerKeyContractCode{
			Hash: contractHash,
		},
	})
	require.NoError(t, err)

	// Doesn't exist.
	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	contractID := getContractID(t, sourceAccount, testSalt, StandaloneNetworkPassphrase)
	contractIDHash := xdr.Hash(contractID)
	notFoundKeyB64, err := xdr.MarshalBase64(xdr.LedgerKey{
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
			BodyType:   xdr.ContractEntryBodyTypeDataEntry,
		},
	})
	require.NoError(t, err)

	var keys []string
	keys = append(keys, contractKeyB64)
	keys = append(keys, notFoundKeyB64)
	request := methods.GetLedgerEntriesRequest{
		Keys: keys,
	}

	var result methods.GetLedgerEntriesResponse
	err = client.CallResult(context.Background(), "getLedgerEntries", request, &result)
	assert.NoError(t, err)
	require.Equal(t, 1, len(result.Entries))
	assert.Greater(t, result.LatestLedger, int64(0))

	var firstEntry xdr.LedgerEntryData
	assert.NoError(t, xdr.SafeUnmarshalBase64(result.Entries[0].XDR, &firstEntry))
	assert.Equal(t, testContract, *firstEntry.MustContractCode().Body.Code)
	assert.Equal(t, contractKeyB64, result.Entries[0].Key)
}
