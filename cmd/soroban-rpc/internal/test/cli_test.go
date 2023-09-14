package test

import (
	"crypto/sha256"
	"fmt"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/google/shlex"
	"github.com/stellar/go/keypair"
	"github.com/stellar/go/txnbuild"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/require"
	"gotest.tools/v3/icmd"
)

func TestCLIContractInstall(t *testing.T) {
	NewCLITest(t)
	output := runSuccessfulCLICmd(t, "contract install --wasm "+helloWorldContractPath)
	wasm := getHelloWorldContract(t)
	contractHash := xdr.Hash(sha256.Sum256(wasm))
	require.Contains(t, output, contractHash.HexString())
}

func TestCLIContractInstallAndDeploy(t *testing.T) {
	NewCLITest(t)
	runSuccessfulCLICmd(t, "contract install --wasm "+helloWorldContractPath)
	wasm := getHelloWorldContract(t)
	contractHash := xdr.Hash(sha256.Sum256(wasm))
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt 0 --wasm-hash %s", contractHash.HexString()))
	outputsContractIDInLastLine(t, output)
}

func outputsContractIDInLastLine(t *testing.T, output string) {
	lines := strings.Split(output, "\n")
	nonEmptyLines := make([]string, 0, len(lines))
	for _, l := range lines {
		if l != "" {
			nonEmptyLines = append(nonEmptyLines, l)
		}
	}
	require.GreaterOrEqual(t, len(nonEmptyLines), 1)
	contractID := nonEmptyLines[len(nonEmptyLines)-1]
	require.Len(t, contractID, 56)
	require.Regexp(t, "^C", contractID)
}

func TestCLIContractDeploy(t *testing.T) {
	NewCLITest(t)
	output := deploy(t, helloWorldContractPath, 0)
	outputsContractIDInLastLine(t, output)
}

func TestCLIContractDeployAndInvoke(t *testing.T) {
	NewCLITest(t)
	contractID := runSuccessfulCLICmd(t, "contract deploy --salt=0 --wasm "+helloWorldContractPath)
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- hello --world=world", contractID))
	require.Contains(t, output, `["Hello","world"]`)
}

func TestCLISimulateTransactionRestoreFromPreambleInFootprint(t *testing.T) {
	NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, "contract deploy --salt=0 --wasm "+helloWorldContractPath)
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)
	count = runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "2", count)
	time.Sleep(time.Second * 60)
	count = runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "3", count)
}
func runSuccessfulCLICmd(t *testing.T, cmd string) string {
	res := runCLICommand(t, cmd)
	stdout, stderr := res.Stdout(), res.Stderr()
	require.NoError(t, res.Error, fmt.Sprintf("stderr:\n%s\nstdout:\n%s\n", stderr, stdout))
	println(fmt.Sprintf("stderr:\n%s\nstdout:\n-------\n%s\n-----\n", stderr, stdout))
	return strings.TrimSpace(stdout)
}

func cliCmd(t *testing.T, cmd string) icmd.Cmd {
	args := []string{"run", "-q", "--", "--vv"}
	parsedArgs, err := shlex.Split(cmd)
	require.NoError(t, err, cmd)
	args = append(args, parsedArgs...)
	c := icmd.Command("cargo", args...)
	c.Env = append(os.Environ(),
		fmt.Sprintf("SOROBAN_RPC_URL=http://localhost:%d/", sorobanRPCPort),
		fmt.Sprintf("SOROBAN_NETWORK_PASSPHRASE=%s", StandaloneNetworkPassphrase),
	)
	return c
}

func deploy(t *testing.T, wasmPath string, id uint32) string {
	testSaltHex := "a1"
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --hd-path %d --salt %s --wasm %s", id, testSaltHex, wasmPath))
	contractID := strings.TrimSpace(output)
	return contractID
}

func runCLICommand(t *testing.T, cmd string) *icmd.Result {
	return icmd.RunCmd(cliCmd(t, cmd))
}

func getAccountFromID(t *testing.T, id uint32) string {
	return strings.Trim(runSuccessfulCLICmd(t, fmt.Sprintf("config identity address --hd-path %d", id)), "\n")
}

// func getTestContractIDFromAccountAndSalt(t *testing.T, id uint32) [32]byte {
// 	return getContractID(t, getAccountFromID(t, id), testSalt, StandaloneNetworkPassphrase)
// }

const MILLION string = "1000000"

func NewCLITest(t *testing.T) *Test {
	test := NewTest(t)
	fundAccount(t, test, getAccountFromID(t, 0), MILLION)
	return test
}

func fundAccount(t *testing.T, test *Test, account string, amount string) {
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
			Destination: account,
			Amount:      amount,
		}},
		BaseFee: txnbuild.MinBaseFee,
		Memo:    nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	require.NoError(t, err)
	sendSuccessfulTransaction(t, client, sourceAccount, tx)
}
