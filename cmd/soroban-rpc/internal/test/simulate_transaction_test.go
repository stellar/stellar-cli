package test

import (
	"context"
	"crypto/sha256"
	"fmt"
	"os"
	"path"
	"runtime"
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

var (
	testContract   = []byte("a contract")
	testSalt       = sha256.Sum256([]byte("a1"))
	testContractId = []byte{
		16, 98, 83, 23, 8, 235, 211, 5,
		62, 173, 70, 33, 7, 31, 219, 59,
		180, 75, 106, 249, 139, 196, 156, 192,
		113, 17, 184, 51, 142, 142, 94, 40,
	}
)

func getHelloWorldContract(t *testing.T) []byte {
	_, filename, _, _ := runtime.Caller(0)
	testDirName := path.Dir(filename)
	contractFile := path.Join(testDirName, "../../../../target/wasm32-unknown-unknown/test-wasms/test_hello_world.wasm")
	ret, err := os.ReadFile(contractFile)
	if err != nil {
		t.Fatalf("unable to read test_hello_world.wasm (%v) please run `make build-test-wasms` at the project root directory", err)
	}
	return ret
}

func createInvokeHostOperation(sourceAccount string, contractID xdr.Hash, method string, args ...xdr.ScVal) *txnbuild.InvokeHostFunction {
	methodSymbol := xdr.ScSymbol(method)
	parameters := xdr.ScVec{
		xdr.ScVal{
			Type: xdr.ScValTypeScvAddress,
			Address: &xdr.ScAddress{
				Type:       xdr.ScAddressTypeScAddressTypeContract,
				ContractId: &contractID,
			},
		},
		xdr.ScVal{
			Type: xdr.ScValTypeScvSymbol,
			Sym:  &methodSymbol,
		},
	}
	parameters = append(parameters, args...)
	return &txnbuild.InvokeHostFunction{
		HostFunction: xdr.HostFunction{
			Type:           xdr.HostFunctionTypeHostFunctionTypeInvokeContract,
			InvokeContract: &parameters,
		},
		Auth:          nil,
		SourceAccount: sourceAccount,
	}
}

func createInstallContractCodeOperation(sourceAccount string, contractCode []byte) *txnbuild.InvokeHostFunction {
	return &txnbuild.InvokeHostFunction{
		HostFunction: xdr.HostFunction{
			Type: xdr.HostFunctionTypeHostFunctionTypeUploadContractWasm,
			Wasm: &contractCode,
		},
		SourceAccount: sourceAccount,
	}
}

func createCreateContractOperation(t *testing.T, sourceAccount string, contractCode []byte, networkPassphrase string) *txnbuild.InvokeHostFunction {
	saltParam := xdr.Uint256(testSalt)
	contractHash := xdr.Hash(sha256.Sum256(contractCode))

	sourceAccountID := xdr.MustAddress(sourceAccount)
	return &txnbuild.InvokeHostFunction{
		HostFunction: xdr.HostFunction{
			Type: xdr.HostFunctionTypeHostFunctionTypeCreateContract,
			CreateContract: &xdr.CreateContractArgs{
				ContractIdPreimage: xdr.ContractIdPreimage{
					Type: xdr.ContractIdPreimageTypeContractIdPreimageFromAddress,
					FromAddress: &xdr.ContractIdPreimageFromAddress{
						Address: xdr.ScAddress{
							Type:      xdr.ScAddressTypeScAddressTypeAccount,
							AccountId: &sourceAccountID,
						},
						Salt: saltParam,
					},
				},
				Executable: xdr.ContractExecutable{
					Type:     xdr.ContractExecutableTypeContractExecutableWasm,
					WasmHash: &contractHash,
				},
			},
		},
		Auth:          []xdr.SorobanAuthorizationEntry{},
		SourceAccount: sourceAccount,
	}
}

func getContractID(t *testing.T, sourceAccount string, salt [32]byte, networkPassphrase string) [32]byte {
	sourceAccountID := xdr.MustAddress(sourceAccount)
	preImage := xdr.HashIdPreimage{
		Type: xdr.EnvelopeTypeEnvelopeTypeContractId,
		ContractId: &xdr.HashIdPreimageContractId{
			NetworkId: sha256.Sum256([]byte(networkPassphrase)),
			ContractIdPreimage: xdr.ContractIdPreimage{
				Type: xdr.ContractIdPreimageTypeContractIdPreimageFromAddress,
				FromAddress: &xdr.ContractIdPreimageFromAddress{
					Address: xdr.ScAddress{
						Type:      xdr.ScAddressTypeScAddressTypeAccount,
						AccountId: &sourceAccountID,
					},
					Salt: salt,
				},
			},
		},
	}

	xdrPreImageBytes, err := preImage.MarshalBinary()
	require.NoError(t, err)
	hashedContractID := sha256.Sum256(xdrPreImageBytes)
	return hashedContractID
}

func preflightTransactionParams(t *testing.T, client *jrpc2.Client, params txnbuild.TransactionParams) txnbuild.TransactionParams {
	savedAutoIncrement := params.IncrementSequenceNum
	params.IncrementSequenceNum = false
	tx, err := txnbuild.NewTransaction(params)
	params.IncrementSequenceNum = savedAutoIncrement
	assert.NoError(t, err)
	assert.Len(t, params.Operations, 1)
	txB64, err := tx.Base64()
	assert.NoError(t, err)

	request := methods.SimulateTransactionRequest{Transaction: txB64}
	var response methods.SimulateTransactionResponse
	err = client.CallResult(context.Background(), "simulateTransaction", request, &response)
	assert.NoError(t, err)
	if !assert.Empty(t, response.Error) {
		fmt.Println(response.Error)
	}
	var transactionData xdr.SorobanTransactionData
	err = xdr.SafeUnmarshalBase64(response.TransactionData, &transactionData)
	assert.NoError(t, err)
	assert.Len(t, response.Results, 1)

	op := params.Operations[0]
	switch v := op.(type) {
	case *txnbuild.InvokeHostFunction:
		v.Ext = xdr.TransactionExt{
			V:           1,
			SorobanData: &transactionData,
		}
		var auth []xdr.SorobanAuthorizationEntry
		for _, b64 := range response.Results[0].Auth {
			var a xdr.SorobanAuthorizationEntry
			err := xdr.SafeUnmarshalBase64(b64, &a)
			assert.NoError(t, err)
			auth = append(auth, a)
		}
		v.Auth = auth
	case *txnbuild.BumpFootprintExpiration:
		v.Ext = xdr.TransactionExt{
			V:           1,
			SorobanData: &transactionData,
		}
	case *txnbuild.RestoreFootprint:
		v.Ext = xdr.TransactionExt{
			V:           1,
			SorobanData: &transactionData,
		}
	default:
		t.Fatalf("Wrong operation type %v", op)
	}

	params.Operations = []txnbuild.Operation{op}

	params.BaseFee += response.MinResourceFee
	return params
}

func TestSimulateTransactionSucceeds(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount: &txnbuild.SimpleAccount{
			AccountID: sourceAccount,
			Sequence:  0,
		},
		IncrementSequenceNum: false,
		Operations: []txnbuild.Operation{
			createInstallContractCodeOperation(sourceAccount, testContract),
		},
		BaseFee: txnbuild.MinBaseFee,
		Memo:    nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	require.NoError(t, err)
	txB64, err := tx.Base64()
	require.NoError(t, err)
	request := methods.SimulateTransactionRequest{Transaction: txB64}

	testContractIdBytes := xdr.ScBytes(testContractId)
	expectedXdr := xdr.ScVal{
		Type:  xdr.ScValTypeScvBytes,
		Bytes: &testContractIdBytes,
	}

	var result methods.SimulateTransactionResponse
	err = client.CallResult(context.Background(), "simulateTransaction", request, &result)
	assert.NoError(t, err)
	assert.Greater(t, result.LatestLedger, int64(0))
	assert.Greater(t, result.Cost.CPUInstructions, uint64(0))
	assert.Greater(t, result.Cost.MemoryBytes, uint64(0))

	expectedTransactionData := xdr.SorobanTransactionData{
		Resources: xdr.SorobanResources{
			Footprint: xdr.LedgerFootprint{
				ReadWrite: []xdr.LedgerKey{
					{
						Type: xdr.LedgerEntryTypeContractCode,
						ContractCode: &xdr.LedgerKeyContractCode{
							Hash:     xdr.Hash(testContractId),
							BodyType: xdr.ContractEntryBodyTypeDataEntry,
						},
					},
				},
			},
			Instructions:              74350,
			ReadBytes:                 40,
			WriteBytes:                112,
			ExtendedMetaDataSizeBytes: 152,
		},
		RefundableFee: 30,
	}

	// First, decode and compare the transaction data so we get a decent diff if it fails.
	var transactionData xdr.SorobanTransactionData
	err = xdr.SafeUnmarshalBase64(result.TransactionData, &transactionData)
	assert.NoError(t, err)
	assert.Equal(t, expectedTransactionData, transactionData)

	// Then decode and check the result xdr, separately so we get a decent diff if it fails.
	assert.Len(t, result.Results, 1)
	var resultXdr xdr.ScVal
	err = xdr.SafeUnmarshalBase64(result.Results[0].XDR, &resultXdr)
	assert.NoError(t, err)
	assert.Equal(t, expectedXdr, resultXdr)

	// test operation which does not have a source account
	withoutSourceAccountOp := createInstallContractCodeOperation("", testContract)
	tx, err = txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount: &txnbuild.SimpleAccount{
			AccountID: sourceAccount,
			Sequence:  0,
		},
		IncrementSequenceNum: false,
		Operations:           []txnbuild.Operation{withoutSourceAccountOp},
		BaseFee:              txnbuild.MinBaseFee,
		Memo:                 nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	require.NoError(t, err)
	txB64, err = tx.Base64()
	require.NoError(t, err)
	request = methods.SimulateTransactionRequest{Transaction: txB64}

	var resultForRequestWithoutOpSource methods.SimulateTransactionResponse
	err = client.CallResult(context.Background(), "simulateTransaction", request, &resultForRequestWithoutOpSource)
	assert.NoError(t, err)
	assert.Equal(t, result, resultForRequestWithoutOpSource)

	// test that operation source account takes precedence over tx source account
	tx, err = txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount: &txnbuild.SimpleAccount{
			AccountID: keypair.Root("test passphrase").Address(),
			Sequence:  0,
		},
		IncrementSequenceNum: false,
		Operations: []txnbuild.Operation{
			createInstallContractCodeOperation(sourceAccount, testContract),
		},
		BaseFee: txnbuild.MinBaseFee,
		Memo:    nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	require.NoError(t, err)
	txB64, err = tx.Base64()
	require.NoError(t, err)
	request = methods.SimulateTransactionRequest{Transaction: txB64}

	var resultForRequestWithDifferentTxSource methods.SimulateTransactionResponse
	err = client.CallResult(context.Background(), "simulateTransaction", request, &resultForRequestWithDifferentTxSource)
	assert.NoError(t, err)
	assert.GreaterOrEqual(t, resultForRequestWithDifferentTxSource.LatestLedger, result.LatestLedger)
	// apart from latest ledger the response should be the same
	resultForRequestWithDifferentTxSource.LatestLedger = result.LatestLedger
	assert.Equal(t, result, resultForRequestWithDifferentTxSource)
}

func TestSimulateInvokeContractTransactionSucceeds(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase)
	address := sourceAccount.Address()
	account := txnbuild.NewSimpleAccount(address, 0)

	helloWorldContract := getHelloWorldContract(t)

	params := preflightTransactionParams(t, client, txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createInstallContractCodeOperation(account.AccountID, helloWorldContract),
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
			createCreateContractOperation(t, address, helloWorldContract, StandaloneNetworkPassphrase),
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
	contractFnParameterSym := xdr.ScSymbol("world")
	authAddrArg := "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H"
	authAccountIDArg := xdr.MustAddress(authAddrArg)
	tx, err = txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			&txnbuild.CreateAccount{
				Destination:   authAddrArg,
				Amount:        "100000",
				SourceAccount: address,
			},
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	assert.NoError(t, err)
	sendSuccessfulTransaction(t, client, sourceAccount, tx)
	params = txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: false,
		Operations: []txnbuild.Operation{
			createInvokeHostOperation(
				address,
				contractID,
				"auth",
				xdr.ScVal{
					Type: xdr.ScValTypeScvAddress,
					Address: &xdr.ScAddress{
						Type:      xdr.ScAddressTypeScAddressTypeAccount,
						AccountId: &authAccountIDArg,
					},
				},
				xdr.ScVal{
					Type: xdr.ScValTypeScvSymbol,
					Sym:  &contractFnParameterSym,
				},
			),
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	}
	tx, err = txnbuild.NewTransaction(params)

	assert.NoError(t, err)

	txB64, err := tx.Base64()
	assert.NoError(t, err)

	request := methods.SimulateTransactionRequest{Transaction: txB64}
	var response methods.SimulateTransactionResponse
	err = client.CallResult(context.Background(), "simulateTransaction", request, &response)
	assert.NoError(t, err)
	assert.Empty(t, response.Error)

	// check the result
	assert.Len(t, response.Results, 1)
	var obtainedResult xdr.ScVal
	err = xdr.SafeUnmarshalBase64(response.Results[0].XDR, &obtainedResult)
	assert.NoError(t, err)
	assert.Equal(t, xdr.ScValTypeScvAddress, obtainedResult.Type)
	require.NotNil(t, obtainedResult.Address)
	assert.Equal(t, authAccountIDArg, obtainedResult.Address.MustAccountId())

	// check the footprint
	var obtainedTransactionData xdr.SorobanTransactionData
	err = xdr.SafeUnmarshalBase64(response.TransactionData, &obtainedTransactionData)
	obtainedFootprint := obtainedTransactionData.Resources.Footprint
	assert.NoError(t, err)
	assert.Len(t, obtainedFootprint.ReadWrite, 1)
	assert.Len(t, obtainedFootprint.ReadOnly, 3)
	ro0 := obtainedFootprint.ReadOnly[0]
	assert.Equal(t, xdr.LedgerEntryTypeAccount, ro0.Type)
	assert.Equal(t, authAddrArg, ro0.Account.AccountId.Address())
	ro1 := obtainedFootprint.ReadOnly[1]
	assert.Equal(t, xdr.LedgerEntryTypeContractData, ro1.Type)
	assert.Equal(t, xdr.ScAddressTypeScAddressTypeContract, ro1.ContractData.Contract.Type)
	assert.Equal(t, xdr.Hash(contractID), *ro1.ContractData.Contract.ContractId)
	assert.Equal(t, xdr.ScValTypeScvLedgerKeyContractInstance, ro1.ContractData.Key.Type)
	ro2 := obtainedFootprint.ReadOnly[2]
	assert.Equal(t, xdr.LedgerEntryTypeContractCode, ro2.Type)
	contractHash := sha256.Sum256(helloWorldContract)
	assert.Equal(t, xdr.Hash(contractHash), ro2.ContractCode.Hash)
	assert.NoError(t, err)

	assert.NotZero(t, obtainedTransactionData.RefundableFee)
	assert.NotZero(t, obtainedTransactionData.Resources.ExtendedMetaDataSizeBytes)
	assert.NotZero(t, obtainedTransactionData.Resources.Instructions)
	assert.NotZero(t, obtainedTransactionData.Resources.ReadBytes)
	assert.NotZero(t, obtainedTransactionData.Resources.WriteBytes)

	// check the auth
	assert.Len(t, response.Results[0].Auth, 1)
	var obtainedAuth xdr.SorobanAuthorizationEntry
	err = xdr.SafeUnmarshalBase64(response.Results[0].Auth[0], &obtainedAuth)
	assert.NoError(t, err)
	assert.Equal(t, obtainedAuth.Credentials.Type, xdr.SorobanCredentialsTypeSorobanCredentialsAddress)
	assert.Nil(t, obtainedAuth.Credentials.Address.SignatureArgs)

	assert.NotZero(t, obtainedAuth.Credentials.Address.Nonce)
	assert.Equal(t, xdr.ScAddressTypeScAddressTypeAccount, obtainedAuth.Credentials.Address.Address.Type)
	assert.Equal(t, authAddrArg, obtainedAuth.Credentials.Address.Address.AccountId.Address())

	assert.Equal(t, xdr.SorobanCredentialsTypeSorobanCredentialsAddress, obtainedAuth.Credentials.Type)
	assert.Equal(t, xdr.ScAddressTypeScAddressTypeAccount, obtainedAuth.Credentials.Address.Address.Type)
	assert.Equal(t, authAddrArg, obtainedAuth.Credentials.Address.Address.AccountId.Address())
	assert.Equal(t, xdr.SorobanAuthorizedFunctionTypeSorobanAuthorizedFunctionTypeContractFn, obtainedAuth.RootInvocation.Function.Type)
	assert.Equal(t, xdr.ScSymbol("auth"), obtainedAuth.RootInvocation.Function.ContractFn.FunctionName)
	assert.Len(t, obtainedAuth.RootInvocation.Function.ContractFn.Args, 2)
	world := obtainedAuth.RootInvocation.Function.ContractFn.Args[1]
	assert.Equal(t, xdr.ScValTypeScvSymbol, world.Type)
	assert.Equal(t, xdr.ScSymbol("world"), *world.Sym)
	assert.Nil(t, obtainedAuth.RootInvocation.SubInvocations)

	// check the events. There will be 2 debug events and the event emitted by the "auth" function
	// which is the one we are going to check.
	assert.Len(t, response.Events, 3)
	var event xdr.DiagnosticEvent
	err = xdr.SafeUnmarshalBase64(response.Events[1], &event)
	assert.NoError(t, err)
	assert.True(t, event.InSuccessfulContractCall)
	assert.NotNil(t, event.Event.ContractId)
	assert.Equal(t, xdr.Hash(contractID), *event.Event.ContractId)
	assert.Equal(t, xdr.ContractEventTypeContract, event.Event.Type)
	assert.Equal(t, int32(0), event.Event.Body.V)
	assert.Equal(t, xdr.ScValTypeScvSymbol, event.Event.Body.V0.Data.Type)
	assert.Equal(t, xdr.ScSymbol("world"), *event.Event.Body.V0.Data.Sym)
	assert.Len(t, event.Event.Body.V0.Topics, 1)
	assert.Equal(t, xdr.ScValTypeScvString, event.Event.Body.V0.Topics[0].Type)
	assert.Equal(t, xdr.ScString("auth"), *event.Event.Body.V0.Topics[0].Str)

	metrics := getMetrics(test)
	require.Contains(t, metrics, "soroban_rpc_json_rpc_request_duration_seconds_count{endpoint=\"simulateTransaction\",status=\"ok\"} 3")
	require.Contains(t, metrics, "soroban_rpc_preflight_pool_request_ledger_get_duration_seconds_count{status=\"ok\",type=\"db\"} 3")
	require.Contains(t, metrics, "soroban_rpc_preflight_pool_request_ledger_get_duration_seconds_count{status=\"ok\",type=\"all\"} 3")
	require.Contains(t, metrics, "soroban_rpc_preflight_pool_request_ledger_entries_fetched_sum 33")
}

func TestSimulateTransactionError(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	invokeHostOp := createInvokeHostOperation(sourceAccount, xdr.Hash{}, "noMethod")
	invokeHostOp.HostFunction = xdr.HostFunction{
		Type:           xdr.HostFunctionTypeHostFunctionTypeInvokeContract,
		InvokeContract: &xdr.ScVec{},
	}
	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount: &txnbuild.SimpleAccount{
			AccountID: keypair.Root(StandaloneNetworkPassphrase).Address(),
			Sequence:  0,
		},
		IncrementSequenceNum: false,
		Operations:           []txnbuild.Operation{invokeHostOp},
		BaseFee:              txnbuild.MinBaseFee,
		Memo:                 nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	require.NoError(t, err)
	txB64, err := tx.Base64()
	require.NoError(t, err)
	request := methods.SimulateTransactionRequest{Transaction: txB64}

	var result methods.SimulateTransactionResponse
	err = client.CallResult(context.Background(), "simulateTransaction", request, &result)
	assert.NoError(t, err)
	assert.Greater(t, result.LatestLedger, int64(0))
	assert.Contains(t, result.Error, "UnexpectedSize")
}

func TestSimulateTransactionMultipleOperations(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount: &txnbuild.SimpleAccount{
			AccountID: keypair.Root(StandaloneNetworkPassphrase).Address(),
			Sequence:  0,
		},
		IncrementSequenceNum: false,
		Operations: []txnbuild.Operation{
			createInstallContractCodeOperation(sourceAccount, testContract),
			createCreateContractOperation(t, sourceAccount, testContract, StandaloneNetworkPassphrase),
		},
		BaseFee: txnbuild.MinBaseFee,
		Memo:    nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	require.NoError(t, err)
	txB64, err := tx.Base64()
	require.NoError(t, err)
	request := methods.SimulateTransactionRequest{Transaction: txB64}

	var result methods.SimulateTransactionResponse
	err = client.CallResult(context.Background(), "simulateTransaction", request, &result)
	assert.NoError(t, err)
	assert.Equal(
		t,
		methods.SimulateTransactionResponse{
			Error: "Transaction contains more than one operation",
		},
		result,
	)
}

func TestSimulateTransactionWithoutInvokeHostFunction(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount: &txnbuild.SimpleAccount{
			AccountID: keypair.Root(StandaloneNetworkPassphrase).Address(),
			Sequence:  0,
		},
		IncrementSequenceNum: false,
		Operations: []txnbuild.Operation{
			&txnbuild.BumpSequence{BumpTo: 1},
		},
		BaseFee: txnbuild.MinBaseFee,
		Memo:    nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	require.NoError(t, err)
	txB64, err := tx.Base64()
	require.NoError(t, err)
	request := methods.SimulateTransactionRequest{Transaction: txB64}

	var result methods.SimulateTransactionResponse
	err = client.CallResult(context.Background(), "simulateTransaction", request, &result)
	assert.NoError(t, err)
	assert.Equal(
		t,
		methods.SimulateTransactionResponse{
			Error: "Transaction contains unsupported operation type: OperationTypeBumpSequence",
		},
		result,
	)
}

func TestSimulateTransactionUnmarshalError(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	request := methods.SimulateTransactionRequest{Transaction: "invalid"}
	var result methods.SimulateTransactionResponse
	err := client.CallResult(context.Background(), "simulateTransaction", request, &result)
	assert.NoError(t, err)
	assert.Equal(
		t,
		"Could not unmarshal transaction",
		result.Error,
	)
}

func TestSimulateTransactionBumpFootprint(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase)
	address := sourceAccount.Address()
	account := txnbuild.NewSimpleAccount(address, 0)

	helloWorldContract := getHelloWorldContract(t)

	params := preflightTransactionParams(t, client, txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createInstallContractCodeOperation(account.AccountID, helloWorldContract),
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
			createCreateContractOperation(t, address, helloWorldContract, StandaloneNetworkPassphrase),
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
	params = preflightTransactionParams(t, client, txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createInvokeHostOperation(
				address,
				contractID,
				"inc",
			),
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	tx, err = txnbuild.NewTransaction(params)
	assert.NoError(t, err)
	sendSuccessfulTransaction(t, client, sourceAccount, tx)

	// get the counter ledger entry
	contractIDHash := xdr.Hash(contractID)
	counterSym := xdr.ScSymbol("COUNTER")
	key := xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeContractData,
		ContractData: &xdr.LedgerKeyContractData{
			Contract: xdr.ScAddress{
				Type:       xdr.ScAddressTypeScAddressTypeContract,
				ContractId: &contractIDHash,
			},
			Key: xdr.ScVal{
				Type: xdr.ScValTypeScvSymbol,
				Sym:  &counterSym,
			},
			Durability: xdr.ContractDataDurabilityTemporary,
			BodyType:   xdr.ContractEntryBodyTypeDataEntry,
		},
	}
	keyB64, err := xdr.MarshalBase64(key)
	require.NoError(t, err)
	getLedgerEntryrequest := methods.GetLedgerEntryRequest{
		Key: keyB64,
	}
	var result methods.GetLedgerEntryResponse
	err = client.CallResult(context.Background(), "getLedgerEntry", getLedgerEntryrequest, &result)
	assert.NoError(t, err)
	var entry xdr.LedgerEntryData
	assert.NoError(t, xdr.SafeUnmarshalBase64(result.XDR, &entry))
	initialExpirationSeq, ok := entry.ExpirationLedgerSeq()
	assert.True(t, ok)

	params = preflightTransactionParams(t, client, txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			&txnbuild.BumpFootprintExpiration{
				LedgersToExpire: 100,
				Ext: xdr.TransactionExt{
					V: 1,
					SorobanData: &xdr.SorobanTransactionData{
						Resources: xdr.SorobanResources{
							Footprint: xdr.LedgerFootprint{
								ReadOnly: []xdr.LedgerKey{key},
							},
						},
					},
				},
			},
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	tx, err = txnbuild.NewTransaction(params)
	assert.NoError(t, err)
	sendSuccessfulTransaction(t, client, sourceAccount, tx)

	err = client.CallResult(context.Background(), "getLedgerEntry", getLedgerEntryrequest, &result)
	assert.NoError(t, err)
	assert.NoError(t, xdr.SafeUnmarshalBase64(result.XDR, &entry))
	newExpirationSeq, ok := entry.ExpirationLedgerSeq()
	assert.True(t, ok)

	assert.Greater(t, newExpirationSeq, initialExpirationSeq)

}
