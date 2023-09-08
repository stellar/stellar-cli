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
	"github.com/stretchr/testify/require"
)

func TestInstallContractWithCLI(t *testing.T) {
	NewCLITest(t)
	output, err := runCLICommand(t, "contract install --wasm ../../../../target/wasm32-unknown-unknown/test-wasms/test_hello_world.wasm")
	require.NoError(t, err)
	wasm := getHelloWorldContract(t)
	contractHash := xdr.Hash(sha256.Sum256(wasm))
	require.Contains(t, string(output), contractHash.HexString())
}

func runCLICommand(t *testing.T, cmd string) ([]byte, error) {
	baseCmdArgs := []string{"run", "--", "--vv"}
	args := strings.Split(cmd, " ")
	args = append(baseCmdArgs, args...)
	c := exec.Command("cargo", args...)
	c.Env = append(os.Environ(),
		fmt.Sprintf("RPC_URL=http://localhost:%d/", sorobanRPCPort),
		fmt.Sprintf("NETWORK_PASPRHASE=%s", StandaloneNetworkPassphrase),
	)
	return c.Output()
}

func NewCLITest(t *testing.T) *Test {
	test := NewTest(t)
	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase)

	// Create default account used byt the CLI
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
