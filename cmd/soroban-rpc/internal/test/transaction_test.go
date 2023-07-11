package test

import (
	"context"
	"fmt"
	"testing"
	"time"

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

func TestSendTransactionSucceedsWithoutResults(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	kp := keypair.Root(StandaloneNetworkPassphrase)
	address := kp.Address()
	account := txnbuild.NewSimpleAccount(address, 0)

	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			&txnbuild.SetOptions{HomeDomain: txnbuild.NewHomeDomain("soroban.com")},
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	assert.NoError(t, err)
	sendSuccessfulTransaction(t, client, kp, tx)
}

func TestSendTransactionSucceedsWithResults(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	kp := keypair.Root(StandaloneNetworkPassphrase)
	address := kp.Address()
	account := txnbuild.NewSimpleAccount(address, 0)

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
	assert.NoError(t, err)
	response := sendSuccessfulTransaction(t, client, kp, tx)

	// Check the result is what we expect
	var transactionResult xdr.TransactionResult
	assert.NoError(t, xdr.SafeUnmarshalBase64(response.ResultXdr, &transactionResult))
	opResults, ok := transactionResult.OperationResults()
	assert.True(t, ok)
	invokeHostFunctionResult, ok := opResults[0].MustTr().GetInvokeHostFunctionResult()
	assert.True(t, ok)
	assert.Equal(t, invokeHostFunctionResult.Code, xdr.InvokeHostFunctionResultCodeInvokeHostFunctionSuccess)
	contractIDBytes := xdr.ScBytes(testContractId)
	expectedScVal := xdr.ScVal{Type: xdr.ScValTypeScvBytes, Bytes: &contractIDBytes}
	var transactionMeta xdr.TransactionMeta
	assert.NoError(t, xdr.SafeUnmarshalBase64(response.ResultMetaXdr, &transactionMeta))
	assert.True(t, expectedScVal.Equals(transactionMeta.V3.SorobanMeta.ReturnValue))
	var resultXdr xdr.TransactionResult
	assert.NoError(t, xdr.SafeUnmarshalBase64(response.ResultXdr, &resultXdr))
	expectedResult := xdr.TransactionResult{
		FeeCharged: resultXdr.FeeCharged,
		Result: xdr.TransactionResultResult{
			Code: xdr.TransactionResultCodeTxSuccess,
			Results: &[]xdr.OperationResult{
				{
					Code: xdr.OperationResultCodeOpInner,
					Tr: &xdr.OperationResultTr{
						Type: xdr.OperationTypeInvokeHostFunction,
						InvokeHostFunctionResult: &xdr.InvokeHostFunctionResult{
							Code:    xdr.InvokeHostFunctionResultCodeInvokeHostFunctionSuccess,
							Success: (*resultXdr.Result.Results)[0].Tr.InvokeHostFunctionResult.Success,
						},
					},
				},
			},
		},
	}

	assert.Equal(t, expectedResult, resultXdr)
}

func TestSendTransactionBadSequence(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	kp := keypair.Root(StandaloneNetworkPassphrase)
	address := kp.Address()
	account := txnbuild.NewSimpleAccount(address, 0)

	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount: &account,
		Operations: []txnbuild.Operation{
			&txnbuild.SetOptions{HomeDomain: txnbuild.NewHomeDomain("soroban.com")},
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	assert.NoError(t, err)
	tx, err = tx.Sign(StandaloneNetworkPassphrase, kp)
	assert.NoError(t, err)
	b64, err := tx.Base64()
	assert.NoError(t, err)

	request := methods.SendTransactionRequest{Transaction: b64}
	var result methods.SendTransactionResponse
	err = client.CallResult(context.Background(), "sendTransaction", request, &result)
	assert.NoError(t, err)

	assert.NotZero(t, result.LatestLedger)
	assert.NotZero(t, result.LatestLedgerCloseTime)
	expectedHashHex, err := tx.HashHex(StandaloneNetworkPassphrase)
	assert.NoError(t, err)
	assert.Equal(t, expectedHashHex, result.Hash)
	assert.Equal(t, proto.TXStatusError, result.Status)
	var errorResult xdr.TransactionResult
	assert.NoError(t, xdr.SafeUnmarshalBase64(result.ErrorResultXDR, &errorResult))
	assert.Equal(t, xdr.TransactionResultCodeTxBadSeq, errorResult.Result.Code)
}

func TestSendTransactionFailedInLedger(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	kp := keypair.Root(StandaloneNetworkPassphrase)
	address := kp.Address()
	account := txnbuild.NewSimpleAccount(address, 0)

	op := createInstallContractCodeOperation(account.AccountID, testContract)
	// without the presources the tx will fail
	op.Ext = xdr.TransactionExt{
		V:           1,
		SorobanData: &xdr.SorobanTransactionData{},
	}
	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			op,
		},
		BaseFee: txnbuild.MinBaseFee * 1000,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	assert.NoError(t, err)
	tx, err = tx.Sign(StandaloneNetworkPassphrase, kp)
	assert.NoError(t, err)
	b64, err := tx.Base64()
	assert.NoError(t, err)

	request := methods.SendTransactionRequest{Transaction: b64}
	var result methods.SendTransactionResponse
	err = client.CallResult(context.Background(), "sendTransaction", request, &result)
	assert.NoError(t, err)

	expectedHashHex, err := tx.HashHex(StandaloneNetworkPassphrase)
	assert.NoError(t, err)

	assert.Equal(t, expectedHashHex, result.Hash)
	if !assert.Equal(t, proto.TXStatusPending, result.Status) {
		var txResult xdr.TransactionResult
		err := xdr.SafeUnmarshalBase64(result.ErrorResultXDR, &txResult)
		assert.NoError(t, err)
		fmt.Printf("error: %#v\n", txResult)
	}
	assert.NotZero(t, result.LatestLedger)
	assert.NotZero(t, result.LatestLedgerCloseTime)

	response := getTransaction(t, client, expectedHashHex)
	assert.Equal(t, methods.TransactionStatusFailed, response.Status)
	var transactionResult xdr.TransactionResult
	assert.NoError(t, xdr.SafeUnmarshalBase64(response.ResultXdr, &transactionResult))
	assert.Equal(t, xdr.TransactionResultCodeTxFailed, transactionResult.Result.Code)
	assert.Greater(t, response.Ledger, result.LatestLedger)
	assert.Greater(t, response.LedgerCloseTime, result.LatestLedgerCloseTime)
	assert.GreaterOrEqual(t, response.LatestLedger, response.Ledger)
	assert.GreaterOrEqual(t, response.LatestLedgerCloseTime, response.LedgerCloseTime)
}

func TestSendTransactionFailedInvalidXDR(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	request := methods.SendTransactionRequest{Transaction: "abcdef"}
	var response methods.SendTransactionResponse
	jsonRPCErr := client.CallResult(context.Background(), "sendTransaction", request, &response).(*jrpc2.Error)
	assert.Equal(t, "invalid_xdr", jsonRPCErr.Message)
	assert.Equal(t, code.InvalidParams, jsonRPCErr.Code)
}

func sendSuccessfulTransaction(t *testing.T, client *jrpc2.Client, kp *keypair.Full, transaction *txnbuild.Transaction) methods.GetTransactionResponse {
	tx, err := transaction.Sign(StandaloneNetworkPassphrase, kp)
	assert.NoError(t, err)
	b64, err := tx.Base64()
	assert.NoError(t, err)

	request := methods.SendTransactionRequest{Transaction: b64}
	var result methods.SendTransactionResponse
	err = client.CallResult(context.Background(), "sendTransaction", request, &result)
	assert.NoError(t, err)

	expectedHashHex, err := tx.HashHex(StandaloneNetworkPassphrase)
	assert.NoError(t, err)

	assert.Equal(t, expectedHashHex, result.Hash)
	if !assert.Equal(t, proto.TXStatusPending, result.Status) {
		var txResult xdr.TransactionResult
		err := xdr.SafeUnmarshalBase64(result.ErrorResultXDR, &txResult)
		assert.NoError(t, err)
		fmt.Printf("error: %#v\n", txResult)
	}
	assert.NotZero(t, result.LatestLedger)
	assert.NotZero(t, result.LatestLedgerCloseTime)

	response := getTransaction(t, client, expectedHashHex)
	if !assert.Equal(t, methods.TransactionStatusSuccess, response.Status) {
		var txResult xdr.TransactionResult
		err := xdr.SafeUnmarshalBase64(response.ResultXdr, &txResult)
		assert.NoError(t, err)
		fmt.Printf("error: %#v\n", txResult)
		var txMeta xdr.TransactionMeta
		err = xdr.SafeUnmarshalBase64(response.ResultMetaXdr, &txMeta)
		assert.NoError(t, err)
		fmt.Printf("meta: %#v\n", txMeta)
	}
	require.NotNil(t, response.ResultXdr)
	assert.Greater(t, response.Ledger, result.LatestLedger)
	assert.Greater(t, response.LedgerCloseTime, result.LatestLedgerCloseTime)
	assert.GreaterOrEqual(t, response.LatestLedger, response.Ledger)
	assert.GreaterOrEqual(t, response.LatestLedgerCloseTime, response.LedgerCloseTime)
	return response
}

func getTransaction(t *testing.T, client *jrpc2.Client, hash string) methods.GetTransactionResponse {
	var result methods.GetTransactionResponse
	for i := 0; i < 60; i++ {
		request := methods.GetTransactionRequest{Hash: hash}
		err := client.CallResult(context.Background(), "getTransaction", request, &result)
		assert.NoError(t, err)

		if result.Status == methods.TransactionStatusNotFound {
			time.Sleep(time.Second)
			continue
		}

		return result
	}
	t.Fatal("getTransaction timed out")
	return result
}
