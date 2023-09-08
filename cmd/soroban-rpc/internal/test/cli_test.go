package test

import (
	"crypto/sha256"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stellar/go/keypair"
	"github.com/stellar/go/txnbuild"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestCLIContractInstall(t *testing.T) {
	NewCLITest(t)
	output, err := runCLICommand("contract install --wasm " + helloWorldContractPath)
	assert.NoError(t, err)
	wasm := getHelloWorldContract(t)
	contractHash := xdr.Hash(sha256.Sum256(wasm))
	require.Contains(t, output, contractHash.HexString())
}

func TestCLIContractDeploy(t *testing.T) {
	NewCLITest(t)
	output, err := runCLICommand("contract deploy --salt 0 --wasm " + helloWorldContractPath)
	println(string(output))
	assert.NoError(t, err)
	require.Contains(t, output, "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM")
}

func TestCLIContractInvokeWithWasm(t *testing.T) {
	NewCLITest(t)
	output, err := runCLICommand(fmt.Sprintf("contract invoke --id 1 --wasm %s -- hello --world=world", helloWorldContractPath))
	assert.NoError(t, err)
	require.Contains(t, output, `["Hello","world"]`)
}

func TestCLIContractDeployAndInvoke(t *testing.T) {
	NewCLITest(t)
	output, err := runCLICommand("contract deploy --id 1 --wasm " + helloWorldContractPath)
	assert.NoError(t, err)
	contractID := strings.TrimSpace(output)
	output, err = runCLICommand(fmt.Sprintf("contract invoke --id %s -- hello --world=world", contractID))
	assert.NoError(t, err)
	require.Contains(t, output, `["Hello","world"]`)
}

func runCLICommand(cmd string) (string, error) {
	baseCmdArgs := []string{"run", "-q", "--", "--vv"}
	args := strings.Split(cmd, " ")
	args = append(baseCmdArgs, args...)
	c := exec.Command("cargo", args...)
	c.Env = append(os.Environ(),
		fmt.Sprintf("SOROBAN_RPC_URL=http://localhost:%d/", sorobanRPCPort),
		fmt.Sprintf("SOROBAN_NETWORK_PASSPHRASE=%s", StandaloneNetworkPassphrase),
	)
	bin, err := c.CombinedOutput()
	return string(bin), err
}

func NewCLITest(t *testing.T) *Test {
	test := NewTest(t)
	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase)

	// Create default account used by the CLI
	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount: &txnbuild.SimpleAccount{
			AccountID: keypair.Root(StandaloneNetworkPassphrase).Address(),
			Sequence:  1,
		},
		IncrementSequenceNum: false,
		Operations: []txnbuild.Operation{&txnbuild.CreateAccount{
			Destination: "GDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCR4W4",
			Amount:      "100000",
		}},
		BaseFee: txnbuild.MinBaseFee,
		Memo:    nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	require.NoError(t, err)
	sendSuccessfulTransaction(t, client, sourceAccount, tx)
	return test
}
