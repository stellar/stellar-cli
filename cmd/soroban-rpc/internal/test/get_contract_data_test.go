package test

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
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

func TestGetContractDataNotFound(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	keyB64, err := xdr.MarshalBase64(getContractCodeLedgerKey())
	require.NoError(t, err)
	contractID := getContractID(t, sourceAccount, testSalt, StandaloneNetworkPassphrase)
	request := methods.GetContractDataRequest{
		ContractID: hex.EncodeToString(contractID[:]),
		Key:        keyB64,
	}

	var result methods.GetContractDataResponse
	jsonRPCErr := client.CallResult(context.Background(), "getContractData", request, &result).(*jrpc2.Error)
	assert.Equal(t, "not found", jsonRPCErr.Message)
	assert.Equal(t, code.InvalidRequest, jsonRPCErr.Code)
}

func TestGetContractDataInvalidParams(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	keyB64, err := xdr.MarshalBase64(getContractCodeLedgerKey())
	require.NoError(t, err)
	request := methods.GetContractDataRequest{
		ContractID: "<>@@#$",
		Key:        keyB64,
	}

	var result methods.GetContractDataResponse
	jsonRPCErr := client.CallResult(context.Background(), "getContractData", request, &result).(*jrpc2.Error)
	assert.Equal(t, "cannot unmarshal contract id", jsonRPCErr.Message)
	assert.Equal(t, code.InvalidParams, jsonRPCErr.Code)

	request.ContractID = "11"
	jsonRPCErr = client.CallResult(context.Background(), "getContractData", request, &result).(*jrpc2.Error)
	assert.Equal(t, "contract id is not 32 bytes", jsonRPCErr.Message)
	assert.Equal(t, code.InvalidParams, jsonRPCErr.Code)

	contractID := getContractID(t, keypair.Root(StandaloneNetworkPassphrase).Address(), testSalt, StandaloneNetworkPassphrase)
	request.ContractID = hex.EncodeToString(contractID[:])
	request.Key = "@#$!@#!@#"
	jsonRPCErr = client.CallResult(context.Background(), "getContractData", request, &result).(*jrpc2.Error)
	assert.Equal(t, "cannot unmarshal key value", jsonRPCErr.Message)
	assert.Equal(t, code.InvalidParams, jsonRPCErr.Code)
}

func TestGetContractDataDeadlineError(t *testing.T) {
	test := NewTest(t)
	test.coreClient.HTTP = &http.Client{
		Timeout: time.Microsecond,
	}

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	keyB64, err := xdr.MarshalBase64(getContractCodeLedgerKey())
	require.NoError(t, err)
	contractID := getContractID(t, sourceAccount, testSalt, StandaloneNetworkPassphrase)
	request := methods.GetContractDataRequest{
		ContractID: hex.EncodeToString(contractID[:]),
		Key:        keyB64,
	}

	var result methods.GetContractDataResponse
	jsonRPCErr := client.CallResult(context.Background(), "getContractData", request, &result).(*jrpc2.Error)
	assert.Equal(t, "could not submit request to core", jsonRPCErr.Message)
	assert.Equal(t, code.InternalError, jsonRPCErr.Code)
}

func TestGetContractDataSucceeds(t *testing.T) {
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

	keyB64, err := xdr.MarshalBase64(getContractCodeLedgerKey())
	require.NoError(t, err)
	contractID := getContractID(t, kp.Address(), testSalt, StandaloneNetworkPassphrase)
	request := methods.GetContractDataRequest{
		ContractID: hex.EncodeToString(contractID[:]),
		Key:        keyB64,
	}

	var result methods.GetContractDataResponse
	err = client.CallResult(context.Background(), "getContractData", request, &result)
	assert.NoError(t, err)
	assert.Greater(t, result.LatestLedger, int64(0))
	assert.GreaterOrEqual(t, result.LatestLedger, result.LastModifiedLedger)
	var scVal xdr.ScVal
	assert.NoError(t, xdr.SafeUnmarshalBase64(result.XDR, &scVal))

	installContractCodeArgs, err := xdr.InstallContractCodeArgs{Code: testContract}.MarshalBinary()
	assert.NoError(t, err)
	contractHash := sha256.Sum256(installContractCodeArgs)
	assert.Equal(t, xdr.Hash(contractHash), scVal.MustObj().MustContractCode().MustWasmId())
}

func assertSendTransaction(t *testing.T, client *jrpc2.Client, kp *keypair.Full, txnParams txnbuild.TransactionParams) {
	tx, err := txnbuild.NewTransaction(txnParams)
	assert.NoError(t, err)
	tx, err = tx.Sign(StandaloneNetworkPassphrase, kp)
	assert.NoError(t, err)
	b64, err := tx.Base64()
	assert.NoError(t, err)

	sendTxRequest := methods.SendTransactionRequest{Transaction: b64}
	var sendTxResponse methods.SendTransactionResponse
	err = client.CallResult(context.Background(), "sendTransaction", sendTxRequest, &sendTxResponse)
	assert.NoError(t, err)
	assert.Equal(t, methods.TransactionPending, sendTxResponse.Status)

	txStatusResponse := getTransactionStatus(t, client, sendTxResponse.ID)
	errorMessage := ""
	if txStatusResponse.Error != nil {
		errorMessage = txStatusResponse.Error.Message
	}
	assert.Equal(t, methods.TransactionSuccess, txStatusResponse.Status, errorMessage)
}
