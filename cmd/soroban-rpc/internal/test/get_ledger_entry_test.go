package test

import (
	"context"
	"net/http"
	"testing"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/code"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/stellar/go/keypair"
	"github.com/stellar/go/txnbuild"
	"github.com/stellar/go/xdr"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

func TestGetLedgerEntryNotFound(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	contractID := getContractID(t, sourceAccount, testSalt)
	keyB64, err := xdr.MarshalBase64(xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeContractData,
		ContractData: &xdr.LedgerKeyContractData{
			ContractId: contractID,
			Key:        getContractCodeLedgerKey(),
		},
	})
	require.NoError(t, err)
	request := methods.GetLedgerEntryRequest{
		Key: keyB64,
	}

	var result methods.GetLedgerEntryResponse
	jsonRPCErr := client.CallResult(context.Background(), "getLedgerEntry", request, &result).(*jrpc2.Error)
	assert.Equal(t, "not found", jsonRPCErr.Message)
	assert.Equal(t, code.InvalidRequest, jsonRPCErr.Code)
}

func TestGetLedgerEntryInvalidParams(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	request := methods.GetLedgerEntryRequest{
		Key: "<>@@#$",
	}

	var result methods.GetLedgerEntryResponse
	jsonRPCErr := client.CallResult(context.Background(), "getLedgerEntry", request, &result).(*jrpc2.Error)
	assert.Equal(t, "cannot unmarshal key value", jsonRPCErr.Message)
	assert.Equal(t, code.InvalidParams, jsonRPCErr.Code)
}

func TestGetLedgerEntryDeadlineError(t *testing.T) {
	test := NewTest(t)
	test.coreClient.HTTP = &http.Client{
		Timeout: time.Microsecond,
	}

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	contractID := getContractID(t, sourceAccount, testSalt)
	keyB64, err := xdr.MarshalBase64(xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeContractData,
		ContractData: &xdr.LedgerKeyContractData{
			ContractId: contractID,
			Key:        getContractCodeLedgerKey(),
		},
	})
	require.NoError(t, err)
	request := methods.GetLedgerEntryRequest{
		Key: keyB64,
	}

	var result methods.GetLedgerEntryResponse
	jsonRPCErr := client.CallResult(context.Background(), "getLedgerEntry", request, &result).(*jrpc2.Error)
	assert.Equal(t, "could not submit request to core", jsonRPCErr.Message)
	assert.Equal(t, code.InternalError, jsonRPCErr.Code)
}

func TestGetLedgerEntrySucceeds(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	kp := keypair.Root(StandaloneNetworkPassphrase)
	account := txnbuild.NewSimpleAccount(kp.Address(), 0)

	// Install and create the contract first
	for _, op := range []txnbuild.Operation{
		createInstallContractCodeOperation(t, account.AccountID, testContract, true),
		createCreateContractOperation(t, account.AccountID, testContract, StandaloneNetworkPassphrase, true),
	} {
		assertSendTransaction(t, client, kp, txnbuild.TransactionParams{
			SourceAccount:        &account,
			IncrementSequenceNum: true,
			Operations:           []txnbuild.Operation{op},
			BaseFee:              txnbuild.MinBaseFee,
			Preconditions: txnbuild.Preconditions{
				TimeBounds: txnbuild.NewInfiniteTimeout(),
			},
		})
	}

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	contractID := getContractID(t, sourceAccount, testSalt)
	keyB64, err := xdr.MarshalBase64(xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeContractData,
		ContractData: &xdr.LedgerKeyContractData{
			ContractId: contractID,
			Key:        getContractCodeLedgerKey(),
		},
	})
	require.NoError(t, err)
	request := methods.GetLedgerEntryRequest{
		Key: keyB64,
	}

	var result methods.GetLedgerEntryResponse
	err = client.CallResult(context.Background(), "getLedgerEntry", request, &result)
	assert.NoError(t, err)
	assert.Greater(t, result.LatestLedger, int64(0))
	assert.GreaterOrEqual(t, result.LatestLedger, result.LastModifiedLedger)
	var scVal xdr.ScVal
	assert.NoError(t, xdr.SafeUnmarshalBase64(result.XDR, &scVal))
	assert.Equal(t, testContract, scVal.MustObj().MustContractCode().MustWasmId())
}
