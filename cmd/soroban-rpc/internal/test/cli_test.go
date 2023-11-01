package test

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"strconv"
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

func cargoTest(t *testing.T, name string) {
	NewCLITest(t)
	c := icmd.Command("cargo", "test", "--features", "integration", "--package", "soroban-test", "--test", "it", "--", name, "--exact", "--nocapture")
	c.Env = append(os.Environ(),
		fmt.Sprintf("SOROBAN_RPC_URL=http://localhost:%d/", sorobanRPCPort),
		fmt.Sprintf("SOROBAN_NETWORK_PASSPHRASE=%s", StandaloneNetworkPassphrase),
	)
	res := icmd.RunCmd(c)
	require.NoError(t, res.Error, res.Stdout(), res.Stderr())
}

func TestCLICargoTest(t *testing.T) {
	names := icmd.RunCmd(icmd.Command("cargo", "-q", "test", "integration::", "--package", "soroban-test", "--features", "integration", "--", "--list"))
	input := names.Stdout()
	lines := strings.Split(strings.TrimSpace(input), "\n")
	for _, line := range lines {
		testName := strings.TrimSuffix(line, ": test")
		t.Run(testName, func(t *testing.T) {
			cargoTest(t, testName)
		})
	}
}

func TestCLIWrapCustom(t *testing.T) {
	NewCLITest(t)
	testAccount := getCLIDefaultAccount(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("lab token wrap --asset=deadbeef:%s", testAccount))
	require.Equal(t, "true", runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id=%s -- authorized --id=%s", strkeyContractID, testAccount)))
	runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id=%s -- mint --to=%s --amount 1", strkeyContractID, testAccount))
}

func TestCLIWrapNative(t *testing.T) {
	NewCLITest(t)
	testAccount := getCLIDefaultAccount(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("lab token wrap --asset=native:%s", testAccount))
	require.Equal(t, "CAMTHSPKXZJIRTUXQP5QWJIFH3XIDMKLFAWVQOFOXPTKAW5GKV37ZC4N", strkeyContractID)
	require.Equal(t, "true", runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id=%s -- authorized --id=%s", strkeyContractID, testAccount)))
	require.Equal(t, "\"9223372036854775807\"", runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id=%s -- balance --id %s", strkeyContractID, testAccount)))
}

func TestCLIContractInstall(t *testing.T) {
	NewCLITest(t)
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract install --wasm %s --ignore-checks", helloWorldContractPath))
	wasm := getHelloWorldContract(t)
	contractHash := xdr.Hash(sha256.Sum256(wasm))
	require.Contains(t, output, contractHash.HexString())
}

func TestCLIContractInstallAndDeploy(t *testing.T) {
	NewCLITest(t)
	runSuccessfulCLICmd(t, fmt.Sprintf("contract install --wasm %s --ignore-checks", helloWorldContractPath))
	wasm := getHelloWorldContract(t)
	contractHash := xdr.Hash(sha256.Sum256(wasm))
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt %s --wasm-hash %s --ignore-checks", hex.EncodeToString(testSalt[:]), contractHash.HexString()))
	outputsContractIDInLastLine(t, output)
}

func TestCLIContractDeploy(t *testing.T) {
	NewCLITest(t)
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt %s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
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
	contractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	output := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- hello --world=world", contractID))
	require.Contains(t, output, `["Hello","world"]`)
}

func TestCLIRestorePreamble(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)
	count = runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "2", count)

	// Wait for the counter ledger entry to expire and successfully invoke the `inc` contract function again
	// This ensures that the CLI restores the entry (using the RestorePreamble in the simulateTransaction response)
	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)
	waitForLedgerEntryToExpire(t, client, getExpirationKeyForCounterLedgerEntry(t, strkeyContractID))

	count = runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "3", count)
}

func TestCLIBump(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	expirationKey := getExpirationKeyForCounterLedgerEntry(t, strkeyContractID)
	initialExpirationSeq := getExpirationForLedgerEntry(t, client, expirationKey)

	bumpOutput := runSuccessfulCLICmd(
		t,
		fmt.Sprintf(
			"contract bump --id %s --key COUNTER --durability persistent --ledgers-to-expire 20",
			strkeyContractID,
		),
	)

	newExpirationSeq := getExpirationForLedgerEntry(t, client, expirationKey)
	assert.Greater(t, newExpirationSeq, initialExpirationSeq)
	assert.Equal(t, fmt.Sprintf("New expiration ledger: %d", newExpirationSeq), bumpOutput)
}
func TestCLIBumpTooLow(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	expirationKey := getExpirationKeyForCounterLedgerEntry(t, strkeyContractID)
	initialExpirationSeq := parseInt(t, getExpirationForLedgerEntry(t, client, expirationKey).GoString())

	bumpOutput := bump(t, strkeyContractID, "400", "--key COUNTER ")

	newExpirationSeq := parseInt(t, getExpirationForLedgerEntry(t, client, expirationKey).GoString())
	assert.Greater(t, newExpirationSeq, initialExpirationSeq)
	assert.Equal(t, newExpirationSeq, bumpOutput)

	updatedExpirationSeq := bump(t, strkeyContractID, "15", "--key COUNTER")
	assert.Equal(t, bumpOutput, updatedExpirationSeq)
}

func TestCLIBumpTooHigh(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	expirationKey := getExpirationKeyForCounterLedgerEntry(t, strkeyContractID)
	initialExpirationSeq := parseInt(t, getExpirationForLedgerEntry(t, client, expirationKey).GoString())

	bumpOutput := bump(t, strkeyContractID, "100000000", "--key COUNTER ")

	newExpirationSeq := parseInt(t, getExpirationForLedgerEntry(t, client, expirationKey).GoString())
	assert.Greater(t, newExpirationSeq, initialExpirationSeq)
	assert.Equal(t, newExpirationSeq, bumpOutput)
}

func TestCLIRestore(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	expirationKey := getExpirationKeyForCounterLedgerEntry(t, strkeyContractID)
	initialExpirationSeq := getExpirationForLedgerEntry(t, client, expirationKey)
	// Wait for the counter ledger entry to expire and successfully invoke the `inc` contract function again
	// This ensures that the CLI restores the entry (using the RestorePreamble in the simulateTransaction response)
	waitForLedgerEntryToExpire(t, client, expirationKey)

	restoreOutput := runSuccessfulCLICmd(
		t,
		fmt.Sprintf(
			"contract restore --id %s --key COUNTER --durability persistent",
			strkeyContractID,
		),
	)

	newExpirationSeq := getExpirationForLedgerEntry(t, client, getExpirationKey(t, getCounterLedgerKey(parseContractStrKey(t, strkeyContractID))))
	assert.Greater(t, newExpirationSeq, initialExpirationSeq)
	assert.Equal(t, fmt.Sprintf("New expiration ledger: %d", newExpirationSeq), restoreOutput)
}

func getExpirationKeyForCounterLedgerEntry(t *testing.T, strkeyContractID string) xdr.LedgerKey {
	return getExpirationKey(t, getCounterLedgerKey(parseContractStrKey(t, strkeyContractID)))
}

func parseContractStrKey(t *testing.T, strkeyContractID string) [32]byte {
	contractIDBytes := strkey.MustDecode(strkey.VersionByteContract, strkeyContractID)
	var contractID [32]byte
	require.Len(t, contractIDBytes, len(contractID))
	copy(contractID[:], contractIDBytes)
	return contractID
}

func runSuccessfulCLICmd(t *testing.T, cmd string) string {
	res := runCLICommand(t, cmd)
	stdout, stderr := res.Stdout(), res.Stderr()
	outputs := fmt.Sprintf("stderr:\n%s\nstdout:\n%s\n", stderr, stdout)
	require.NoError(t, res.Error, outputs)
	fmt.Print(outputs)
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

func parseInt(t *testing.T, s string) uint64 {
	i, err := strconv.ParseUint(strings.TrimSpace(s), 10, 64)
	require.NoError(t, err)
	return i
}

func bump(t *testing.T, contractId string, amount string, rest string) uint64 {

	res := runSuccessfulCLICmd(
		t,
		fmt.Sprintf(
			"contract bump --expiration-ledger-only --id=%s --durability persistent --ledgers-to-expire=%s %s",
			contractId,
			amount,
			rest,
		),
	)

	return parseInt(t, res)
}
