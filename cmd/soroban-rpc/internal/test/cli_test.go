package test

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"strings"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/google/shlex"
	"github.com/stellar/go/keypair"
	"github.com/stellar/go/strkey"
	"github.com/stellar/go/txnbuild"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"
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
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt %s --wasm-hash %s", hex.EncodeToString(testSalt[:]), contractHash.HexString()))
	outputsContractIDInLastLine(t, output)
}

func TestCLIContractDeploy(t *testing.T) {
	NewCLITest(t)
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt %s --wasm %s", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
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

func TestCLIContractDeployAndInvoke(t *testing.T) {
	NewCLITest(t)
	contractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- hello --world=world", contractID))
	require.Contains(t, output, `["Hello","world"]`)
}

func TestCLIRestorePreamble(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)
	count = runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "2", count)

	// Wait for the counter ledger entry to expire and successfully invoke the `inc` contract function again
	// This ensures that the CLI restores the entry (using the RestorePreamble in the simulateTransaction response)
	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)
	contractIDBytes := strkey.MustDecode(strkey.VersionByteContract, strkeyContractID)
	require.Len(t, contractIDBytes, 32)
	var contractID [32]byte
	copy(contractID[:], contractIDBytes)
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
			Durability: xdr.ContractDataDurabilityPersistent,
		},
	}

	binKey, err := key.MarshalBinary()
	assert.NoError(t, err)

	expiration := xdr.LedgerKeyExpiration{
		KeyHash: sha256.Sum256(binKey),
	}
	waitForLedgerEntryToExpire(t, client, expiration)

	count = runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "3", count)
}

func runSuccessfulCLICmd(t *testing.T, cmd string) string {
	res := runCLICommand(t, cmd)
	stdout, stderr := res.Stdout(), res.Stderr()
	outputs := fmt.Sprintf("stderr:\n%s\nstdout:\n%s\n", stderr, stdout)
	require.NoError(t, res.Error, outputs)
	fmt.Printf(outputs)
	return strings.TrimSpace(stdout)
}

func runCLICommand(t *testing.T, cmd string) *icmd.Result {
	args := []string{"run", "-q", "--", "--vv"}
	parsedArgs, err := shlex.Split(cmd)
	require.NoError(t, err, cmd)
	args = append(args, parsedArgs...)
	c := icmd.Command("cargo", args...)
	c.Env = append(os.Environ(),
		fmt.Sprintf("SOROBAN_RPC_URL=http://localhost:%d/", sorobanRPCPort),
		fmt.Sprintf("SOROBAN_NETWORK_PASSPHRASE=%s", StandaloneNetworkPassphrase),
	)
	return icmd.RunCmd(c)
}

func getCLIDefaultAccount(t *testing.T) string {
	return runSuccessfulCLICmd(t, "config identity address --hd-path 0")
}

func NewCLITest(t *testing.T) *Test {
	test := NewTest(t)
	fundAccount(t, test, getCLIDefaultAccount(t), "1000000")
	return test
}

func fundAccount(t *testing.T, test *Test, account string, amount string) {
	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	sourceAccount := keypair.Root(StandaloneNetworkPassphrase)

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
