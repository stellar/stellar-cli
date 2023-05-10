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
		234, 159, 203, 129, 174, 84, 162, 159,
		107, 59, 242, 147, 132, 125, 63, 215,
		233, 163, 105, 253, 28, 128, 172, 175,
		236, 106, 189, 87, 19, 23, 224, 194,
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

func createInvokeHostOperation(sourceAccount string, ext xdr.TransactionExt, contractID xdr.Hash, method string, args ...xdr.ScVal) *txnbuild.InvokeHostFunctions {
	var contractIDBytes = xdr.ScBytes(contractID[:])
	methodSymbol := xdr.ScSymbol(method)
	parameters := xdr.ScVec{
		xdr.ScVal{
			Type:  xdr.ScValTypeScvBytes,
			Bytes: &contractIDBytes,
		},
		xdr.ScVal{
			Type: xdr.ScValTypeScvSymbol,
			Sym:  &methodSymbol,
		},
	}
	parameters = append(parameters, args...)
	return &txnbuild.InvokeHostFunctions{

		Functions: []xdr.HostFunction{
			{
				Args: xdr.HostFunctionArgs{
					Type:           xdr.HostFunctionTypeHostFunctionTypeInvokeContract,
					InvokeContract: &parameters,
				},
			},
		},
		Ext:           ext,
		SourceAccount: sourceAccount,
	}
}

func createInstallContractCodeOperation(t *testing.T, sourceAccount string, contractCode []byte, includeExt bool) *txnbuild.InvokeHostFunctions {
	var ext xdr.TransactionExt
	if includeExt {
		uploadContractCodeArgs, err := xdr.UploadContractWasmArgs{Code: contractCode}.MarshalBinary()
		assert.NoError(t, err)
		contractHash := sha256.Sum256(uploadContractCodeArgs)
		footprint := xdr.LedgerFootprint{
			ReadWrite: []xdr.LedgerKey{
				{
					Type: xdr.LedgerEntryTypeContractCode,
					ContractCode: &xdr.LedgerKeyContractCode{
						Hash: contractHash,
					},
				},
			},
		}
		// TODO: fill in this data properly
		//       we will most likely need to invoke the preflight endpoint
		ext = xdr.TransactionExt{
			V: 1,
			SorobanData: &xdr.SorobanTransactionData{
				Resources: xdr.SorobanResources{
					Footprint:                 footprint,
					Instructions:              0,
					ReadBytes:                 0,
					WriteBytes:                0,
					ExtendedMetaDataSizeBytes: 0,
				},
				RefundableFee: 0,
				Ext:           xdr.ExtensionPoint{},
			},
		}
	}

	return &txnbuild.InvokeHostFunctions{
		Ext: ext,
		Functions: []xdr.HostFunction{
			{
				Args: xdr.HostFunctionArgs{
					Type: xdr.HostFunctionTypeHostFunctionTypeUploadContractWasm,
					UploadContractWasm: &xdr.UploadContractWasmArgs{
						Code: contractCode,
					},
				},
				Auth: []xdr.ContractAuth{},
			},
		},
		SourceAccount: sourceAccount,
	}
}

func createCreateContractOperation(t *testing.T, sourceAccount string, contractCode []byte, networkPassphrase string, includeExt bool) *txnbuild.InvokeHostFunctions {
	saltParam := xdr.Uint256(testSalt)

	var ext xdr.TransactionExt
	if includeExt {
		uploadContractCodeArgs, err := xdr.UploadContractWasmArgs{Code: contractCode}.MarshalBinary()
		assert.NoError(t, err)
		contractHash := xdr.Hash(sha256.Sum256(uploadContractCodeArgs))
		footprint := xdr.LedgerFootprint{
			ReadWrite: []xdr.LedgerKey{
				{
					Type: xdr.LedgerEntryTypeContractData,
					ContractData: &xdr.LedgerKeyContractData{
						ContractId: xdr.Hash(getContractID(t, sourceAccount, testSalt, networkPassphrase)),
						Key: xdr.ScVal{
							Type: xdr.ScValTypeScvLedgerKeyContractExecutable,
						},
					},
				},
			},
			ReadOnly: []xdr.LedgerKey{
				{
					Type: xdr.LedgerEntryTypeContractCode,
					ContractCode: &xdr.LedgerKeyContractCode{
						Hash: contractHash,
					},
				},
			},
		}
		// TODO: fill in this data properly
		//       we will most likely need to invoke the preflight endpoint
		ext = xdr.TransactionExt{
			V: 1,
			SorobanData: &xdr.SorobanTransactionData{
				Resources: xdr.SorobanResources{
					Footprint:                 footprint,
					Instructions:              0,
					ReadBytes:                 0,
					WriteBytes:                0,
					ExtendedMetaDataSizeBytes: 0,
				},
				RefundableFee: 0,
				Ext:           xdr.ExtensionPoint{},
			},
		}
	}

	uploadContractCodeArgs, err := xdr.UploadContractWasmArgs{Code: contractCode}.MarshalBinary()
	assert.NoError(t, err)
	contractHash := xdr.Hash(sha256.Sum256(uploadContractCodeArgs))

	return &txnbuild.InvokeHostFunctions{
		Ext: ext,

		Functions: []xdr.HostFunction{
			{
				Args: xdr.HostFunctionArgs{
					Type: xdr.HostFunctionTypeHostFunctionTypeCreateContract,
					CreateContract: &xdr.CreateContractArgs{
						ContractId: xdr.ContractId{
							Type: xdr.ContractIdTypeContractIdFromSourceAccount,
							Salt: &saltParam,
						},
						Executable: xdr.ScContractExecutable{
							Type:   xdr.ScContractExecutableTypeSccontractExecutableWasmRef,
							WasmId: &contractHash,
						},
					},
				},
				Auth: []xdr.ContractAuth{},
			},
		},
		SourceAccount: sourceAccount,
	}
}

func getContractID(t *testing.T, sourceAccount string, salt [32]byte, networkPassphrase string) [32]byte {
	networkID := xdr.Hash(sha256.Sum256([]byte(networkPassphrase)))
	preImage := xdr.HashIdPreimage{
		Type: xdr.EnvelopeTypeEnvelopeTypeContractIdFromSourceAccount,
		SourceAccountContractId: &xdr.HashIdPreimageSourceAccountContractId{
			NetworkId: networkID,
			Salt:      salt,
		},
	}
	if err := preImage.SourceAccountContractId.SourceAccount.SetAddress(sourceAccount); err != nil {
		t.Errorf("failed to set address : %v", err)
		t.FailNow()
	}
	xdrPreImageBytes, err := preImage.MarshalBinary()
	require.NoError(t, err)
	hashedContractID := sha256.Sum256(xdrPreImageBytes)
	return hashedContractID
}

func preflightTransactionParams(t *testing.T, client *jrpc2.Client, params txnbuild.TransactionParams) (txnbuild.TransactionParams, methods.SimulateTransactionResponse) {
	savedAutoIncrement := params.IncrementSequenceNum

	params.IncrementSequenceNum = false
	tx, err := txnbuild.NewTransaction(params)
	params.IncrementSequenceNum = savedAutoIncrement
	assert.NoError(t, err)
	assert.Len(t, params.Operations, 1)
	op, ok := params.Operations[0].(*txnbuild.InvokeHostFunctions)
	assert.True(t, ok)
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
	op.Ext = xdr.TransactionExt{
		V:           1,
		SorobanData: &transactionData,
	}
	for i, res := range response.Results {
		var auth []xdr.ContractAuth
		for _, b64 := range res.Auth {
			var a xdr.ContractAuth
			err := xdr.SafeUnmarshalBase64(b64, &a)
			assert.NoError(t, err)
			auth = append(auth, a)
		}
		op.Functions[i].Auth = auth
	}

	params.Operations = []txnbuild.Operation{op}
	params.BaseFee += response.MinResourceFee
	return params, response
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
			createInstallContractCodeOperation(t, sourceAccount, testContract, false),
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
	expectedXdr, err := xdr.MarshalBase64(xdr.ScVal{
		Type:  xdr.ScValTypeScvBytes,
		Bytes: &testContractIdBytes,
	})
	require.NoError(t, err)

	var result methods.SimulateTransactionResponse
	err = client.CallResult(context.Background(), "simulateTransaction", request, &result)
	assert.NoError(t, err)
	assert.Greater(t, result.LatestLedger, int64(0))
	assert.Greater(t, result.Cost.CPUInstructions, uint64(0))
	assert.Greater(t, result.Cost.MemoryBytes, uint64(0))
	assert.Equal(
		t,
		methods.SimulateTransactionResponse{
			Cost: methods.SimulateTransactionCost{
				CPUInstructions: result.Cost.CPUInstructions,
				MemoryBytes:     result.Cost.MemoryBytes,
			},
			TransactionData: "AAAAAAAAAAEAAAAH6p/Lga5Uop9rO/KThH0/1+mjaf0cgKyv7Gq9VxMX4MIAAGWMAAAAAAAAAGQAAABkAAAAAAAAABQAAAAA",
			MinResourceFee:  result.MinResourceFee,
			Results: []methods.SimulateHostFunctionResult{
				{
					XDR: expectedXdr,
				},
			},
			LatestLedger: result.LatestLedger,
		},
		result,
	)

	// test operation which does not have a source account
	withoutSourceAccountOp := createInstallContractCodeOperation(t, "", testContract, false)
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
			createInstallContractCodeOperation(t, sourceAccount, testContract, false),
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

	params, _ := preflightTransactionParams(t, client, txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createInstallContractCodeOperation(t, account.AccountID, helloWorldContract, false),
		},
		BaseFee: 2992,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	tx, err := txnbuild.NewTransaction(params)
	assert.NoError(t, err)
	sendSuccessfulTransaction(t, client, sourceAccount, tx)

	tx, err = txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createCreateContractOperation(t, address, helloWorldContract, StandaloneNetworkPassphrase, true),
		},
		// TODO: replace this will the preflight min value?
		BaseFee: txnbuild.MinBaseFee * 1000,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
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
	tx, err = txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createInvokeHostOperation(
				address,
				// TODO: fill in this data properly
				//       we will most likely need to invoke the preflight endpoint
				xdr.TransactionExt{},
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
		// TODO: replace this will the preflight min value?
		BaseFee: txnbuild.MinBaseFee * 1000,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})

	assert.NoError(t, err)
	txB64, err := tx.Base64()
	require.NoError(t, err)
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
	assert.NotNil(t, obtainedResult.Address)
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
	assert.Equal(t, xdr.Hash(contractID), ro1.ContractData.ContractId)
	assert.Equal(t, xdr.ScValTypeScvLedgerKeyContractExecutable, ro1.ContractData.Key.Type)
	ro2 := obtainedFootprint.ReadOnly[2]
	assert.Equal(t, xdr.LedgerEntryTypeContractCode, ro2.Type)
	uploadContractCodeArgs, err := xdr.UploadContractWasmArgs{Code: helloWorldContract}.MarshalBinary()
	assert.NoError(t, err)
	contractHash := sha256.Sum256(uploadContractCodeArgs)
	assert.Equal(t, xdr.Hash(contractHash), ro2.ContractCode.Hash)
	assert.NoError(t, err)

	// TODO: check the other transactiondata fields

	// check the auth
	assert.Len(t, response.Results[0].Auth, 1)
	var obtainedAuth xdr.ContractAuth
	err = xdr.SafeUnmarshalBase64(response.Results[0].Auth[0], &obtainedAuth)
	assert.NoError(t, err)
	assert.Nil(t, obtainedAuth.SignatureArgs)

	assert.Equal(t, xdr.Uint64(0), obtainedAuth.AddressWithNonce.Nonce)
	assert.Equal(t, xdr.ScAddressTypeScAddressTypeAccount, obtainedAuth.AddressWithNonce.Address.Type)
	assert.Equal(t, authAddrArg, obtainedAuth.AddressWithNonce.Address.AccountId.Address())

	assert.Equal(t, xdr.Hash(contractID), obtainedAuth.RootInvocation.ContractId)
	assert.Equal(t, xdr.ScSymbol("auth"), obtainedAuth.RootInvocation.FunctionName)
	assert.Len(t, obtainedAuth.RootInvocation.Args, 2)
	world := obtainedAuth.RootInvocation.Args[1]
	assert.Equal(t, xdr.ScValTypeScvSymbol, world.Type)
	assert.Equal(t, xdr.ScSymbol("world"), *world.Sym)
	assert.Nil(t, obtainedAuth.RootInvocation.SubInvocations)

	// check the events
	assert.Len(t, response.Events, 1)
	var event xdr.DiagnosticEvent
	err = xdr.SafeUnmarshalBase64(response.Events[0], &event)
	assert.NoError(t, err)
	assert.True(t, event.InSuccessfulContractCall)
	assert.Equal(t, xdr.Hash(contractID), *event.Event.ContractId)
	assert.Equal(t, xdr.ContractEventTypeContract, event.Event.Type)
	assert.Equal(t, int32(0), event.Event.Body.V)
	assert.Equal(t, xdr.ScValTypeScvSymbol, event.Event.Body.V0.Data.Type)
	assert.Equal(t, xdr.ScSymbol("world"), *event.Event.Body.V0.Data.Sym)
	assert.Len(t, event.Event.Body.V0.Topics, 1)
	assert.Equal(t, xdr.ScValTypeScvString, event.Event.Body.V0.Topics[0].Type)
	assert.Equal(t, xdr.ScString("auth"), *event.Event.Body.V0.Topics[0].Str)

	metrics := getMetrics(test)
	require.Contains(t, metrics, "soroban_rpc_json_rpc_request_duration_seconds_count{endpoint=\"simulateTransaction\",status=\"ok\"} 1")
	require.Contains(t, metrics, "soroban_rpc_preflight_pool_request_ledger_get_duration_seconds_count{status=\"ok\",type=\"db\"} 1")
	require.Contains(t, metrics, "soroban_rpc_preflight_pool_request_ledger_get_duration_seconds_count{status=\"ok\",type=\"all\"} 1")
	require.Contains(t, metrics, "soroban_rpc_preflight_pool_request_ledger_entries_fetched_sum 4")
}

func TestSimulateTransactionError(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	invokeHostOp := createInvokeHostOperation(sourceAccount, xdr.TransactionExt{}, xdr.Hash{}, "noMethod")
	invokeHostOp.Functions[0].Args.InvokeContract = &xdr.ScVec{}
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
	assert.Empty(t, result.Results)
	assert.Greater(t, result.LatestLedger, int64(0))
	assert.Contains(t, result.Error, "InputArgsWrongLength")
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
			createInstallContractCodeOperation(t, sourceAccount, testContract, false),
			createCreateContractOperation(t, sourceAccount, testContract, StandaloneNetworkPassphrase, false),
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
			Error: "Transaction does not contain invoke host function operation",
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
