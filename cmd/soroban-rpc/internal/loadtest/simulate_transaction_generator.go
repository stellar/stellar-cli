package loadtest

import (
	"os"

	"github.com/creachadair/jrpc2"
	"github.com/stellar/go/keypair"
	"github.com/stellar/go/txnbuild"
	"github.com/stellar/go/xdr"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

// Generates simple simulateTransaction requests to invoke a "hello world" contract.
type SimulateTransactionGenerator struct {
	networkPassphrase      string
	helloWorldContractPath string
}

func (generator *SimulateTransactionGenerator) GenerateSpec() (jrpc2.Spec, error) {
	sourceAccount := keypair.Root(generator.networkPassphrase).Address()
	contractBinary, err := os.ReadFile(generator.helloWorldContractPath)
	if err != nil {
		return jrpc2.Spec{}, err
	}
	invokeHostFunction := &txnbuild.InvokeHostFunction{
		HostFunction: xdr.HostFunction{
			Type: xdr.HostFunctionTypeHostFunctionTypeUploadContractWasm,
			Wasm: &contractBinary,
		},
		SourceAccount: sourceAccount,
	}
	params := txnbuild.TransactionParams{
		SourceAccount: &txnbuild.SimpleAccount{
			AccountID: sourceAccount,
			Sequence:  0,
		},
		IncrementSequenceNum: false,
		Operations: []txnbuild.Operation{
			invokeHostFunction,
		},
		BaseFee: txnbuild.MinBaseFee,
		Memo:    nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	}

	params.IncrementSequenceNum = false
	tx, err := txnbuild.NewTransaction(params)
	if err != nil {
		return jrpc2.Spec{}, err
	}
	txB64, err := tx.Base64()
	if err != nil {
		return jrpc2.Spec{}, err
	}
	return jrpc2.Spec{
		Method: "simulateTransaction",
		Params: methods.SimulateTransactionRequest{Transaction: txB64},
	}, nil
}
