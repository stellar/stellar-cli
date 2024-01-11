package test

import (
	"context"
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

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
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
	it := NewCLITest(t)
	assetCode := "deadbeef"
	issuerAccount := getCLIDefaultAccount(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract asset deploy --asset=%s:%s", assetCode, issuerAccount))
	require.Equal(t, "true", runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id=%s -- authorized --id=%s", strkeyContractID, issuerAccount)))
	asset := txnbuild.CreditAsset{
		Code:   assetCode,
		Issuer: issuerAccount,
	}
	establishAccountTrustline(t, it, it.MasterKey(), it.MasterAccount(), asset)
	masterAccount := keypair.Root(StandaloneNetworkPassphrase).Address()
	runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id=%s -- mint --to=%s --amount 1", strkeyContractID, masterAccount))
}

func TestCLIWrapNative(t *testing.T) {
	NewCLITest(t)
	testAccount := getCLIDefaultAccount(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract asset deploy --asset=native:%s", testAccount))
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

	// Wait for the counter ledger entry to ttl and successfully invoke the `inc` contract function again
	// This ensures that the CLI restores the entry (using the RestorePreamble in the simulateTransaction response)
	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)
	waitUntilLedgerEntryTTL(t, client, getCounterLedgerKey(parseContractStrKey(t, strkeyContractID)))

	count = runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "3", count)
}

func TestCLIExtend(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	ttlKey := getCounterLedgerKey(parseContractStrKey(t, strkeyContractID))
	initialLiveUntilSeq := getLedgerEntryLiveUntil(t, client, ttlKey)

	extendOutput := runSuccessfulCLICmd(
		t,
		fmt.Sprintf(
			"contract extend --id %s --key COUNTER --durability persistent --ledgers-to-extend 20",
			strkeyContractID,
		),
	)

	newLiveUntilSeq := getLedgerEntryLiveUntil(t, client, ttlKey)
	assert.Greater(t, newLiveUntilSeq, initialLiveUntilSeq)
	assert.Equal(t, fmt.Sprintf("New ttl ledger: %d", newLiveUntilSeq), extendOutput)
}
func TestCLIExtendTooLow(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	ttlKey := getCounterLedgerKey(parseContractStrKey(t, strkeyContractID))
	initialLiveUntilSeq := parseInt(t, getLedgerEntryLiveUntil(t, client, ttlKey).GoString())

	extendOutput := extend(t, strkeyContractID, "400", "--key COUNTER ")

	newLiveUntilSeq := parseInt(t, getLedgerEntryLiveUntil(t, client, ttlKey).GoString())
	assert.Greater(t, newLiveUntilSeq, initialLiveUntilSeq)
	assert.Equal(t, newLiveUntilSeq, extendOutput)

	updatedLiveUntilSeq := extend(t, strkeyContractID, "15", "--key COUNTER")
	assert.Equal(t, extendOutput, updatedLiveUntilSeq)
}

func TestCLIExtendTooHigh(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	ttlKey := getCounterLedgerKey(parseContractStrKey(t, strkeyContractID))
	initialLiveUntilSeq := parseInt(t, getLedgerEntryLiveUntil(t, client, ttlKey).GoString())

	extendOutput := extend(t, strkeyContractID, "100000000", "--key COUNTER ")

	newLiveUntilSeq := parseInt(t, getLedgerEntryLiveUntil(t, client, ttlKey).GoString())
	assert.Greater(t, newLiveUntilSeq, initialLiveUntilSeq)
	assert.Equal(t, newLiveUntilSeq, extendOutput)
}

func TestCLIRestore(t *testing.T) {
	test := NewCLITest(t)
	strkeyContractID := runSuccessfulCLICmd(t, fmt.Sprintf("contract deploy --salt=%s --wasm %s --ignore-checks", hex.EncodeToString(testSalt[:]), helloWorldContractPath))
	count := runSuccessfulCLICmd(t, fmt.Sprintf("contract invoke --id %s -- inc", strkeyContractID))
	require.Equal(t, "1", count)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	ttlKey := getCounterLedgerKey(parseContractStrKey(t, strkeyContractID))
	initialLiveUntilSeq := getLedgerEntryLiveUntil(t, client, ttlKey)
	// Wait for the counter ledger entry to ttl and successfully invoke the `inc` contract function again
	// This ensures that the CLI restores the entry (using the RestorePreamble in the simulateTransaction response)
	waitUntilLedgerEntryTTL(t, client, ttlKey)

	restoreOutput := runSuccessfulCLICmd(
		t,
		fmt.Sprintf(
			"contract restore --id %s --key COUNTER --durability persistent",
			strkeyContractID,
		),
	)

	newLiveUntilSeq := getLedgerEntryLiveUntil(t, client, getCounterLedgerKey(parseContractStrKey(t, strkeyContractID)))
	assert.Greater(t, newLiveUntilSeq, initialLiveUntilSeq)
	assert.Equal(t, fmt.Sprintf("New ttl ledger: %d", newLiveUntilSeq), restoreOutput)

	// FIXME: the following checks shouldn't live here:

	// test to see that we get an error when requesting the ttl ledger entry explicitly.
	ledgerTTLEntry := getTtlKey(t, getCounterLedgerKey(parseContractStrKey(t, strkeyContractID)))
	ledgerTTLEntryB64, err := xdr.MarshalBase64(ledgerTTLEntry)
	require.NoError(t, err)
	var getLedgerEntryResult methods.GetLedgerEntryResponse
	err = client.CallResult(context.Background(), "getLedgerEntry", methods.GetLedgerEntryRequest{
		Key: ledgerTTLEntryB64,
	}, &getLedgerEntryResult)
	require.Error(t, err)
	require.Contains(t, err.Error(), methods.ErrLedgerTtlEntriesCannotBeQueriedDirectly)

	// repeat with getLedgerEntries
	var getLedgerEntriesResult methods.GetLedgerEntriesResponse
	err = client.CallResult(context.Background(), "getLedgerEntries", methods.GetLedgerEntriesRequest{
		Keys: []string{ledgerTTLEntryB64},
	}, &getLedgerEntriesResult)
	require.Error(t, err)
	require.Contains(t, err.Error(), methods.ErrLedgerTtlEntriesCannotBeQueriedDirectly)
}

func getTtlKey(t *testing.T, key xdr.LedgerKey) xdr.LedgerKey {
	assert.True(t, key.Type == xdr.LedgerEntryTypeContractCode || key.Type == xdr.LedgerEntryTypeContractData)
	binKey, err := key.MarshalBinary()
	assert.NoError(t, err)
	return xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeTtl,
		Ttl: &xdr.LedgerKeyTtl{
			KeyHash: sha256.Sum256(binKey),
		},
	}
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
		"SOROBAN_ACCOUNT=test",
	)
	return icmd.RunCmd(c)
}

func getCLIDefaultAccount(t *testing.T) string {
	runSuccessfulCLICmd(t, "keys generate -d test --no-fund")
	return "GDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCR4W4"
}

func NewCLITest(t *testing.T) *Test {
	test := NewTest(t)
	fundAccount(t, test, getCLIDefaultAccount(t), "1000000")
	return test
}

func fundAccount(t *testing.T, test *Test, account string, amount string) {
	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount:        test.MasterAccount(),
		IncrementSequenceNum: true,
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
	sendSuccessfulTransaction(t, client, test.MasterKey(), tx)
}

func establishAccountTrustline(t *testing.T, test *Test, kp *keypair.Full, account txnbuild.Account, asset txnbuild.Asset) {
	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	line := asset.MustToChangeTrustAsset()
	tx, err := txnbuild.NewTransaction(txnbuild.TransactionParams{
		SourceAccount:        account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{&txnbuild.ChangeTrust{
			Line:  line,
			Limit: "2000",
		}},
		BaseFee: txnbuild.MinBaseFee,
		Memo:    nil,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	})
	require.NoError(t, err)
	sendSuccessfulTransaction(t, client, kp, tx)
}

func parseInt(t *testing.T, s string) uint64 {
	i, err := strconv.ParseUint(strings.TrimSpace(s), 10, 64)
	require.NoError(t, err)
	return i
}

func extend(t *testing.T, contractId string, amount string, rest string) uint64 {

	res := runSuccessfulCLICmd(
		t,
		fmt.Sprintf(
			"contract extend --ttl-ledger-only --id=%s --durability persistent --ledgers-to-extend=%s %s",
			contractId,
			amount,
			rest,
		),
	)

	return parseInt(t, res)
}

func getLedgerEntryLiveUntil(t *testing.T, client *jrpc2.Client, ttlLedgerKey xdr.LedgerKey) xdr.Uint32 {
	keyB64, err := xdr.MarshalBase64(ttlLedgerKey)
	require.NoError(t, err)
	getLedgerEntryrequest := methods.GetLedgerEntryRequest{
		Key: keyB64,
	}
	var getLedgerEntryResult methods.GetLedgerEntryResponse
	err = client.CallResult(context.Background(), "getLedgerEntry", getLedgerEntryrequest, &getLedgerEntryResult)
	require.NoError(t, err)
	var entry xdr.LedgerEntryData
	require.NoError(t, xdr.SafeUnmarshalBase64(getLedgerEntryResult.XDR, &entry))

	require.Contains(t, []xdr.LedgerEntryType{xdr.LedgerEntryTypeContractCode, xdr.LedgerEntryTypeContractData}, entry.Type)
	require.NotNil(t, getLedgerEntryResult.LiveUntilLedgerSeq)
	return xdr.Uint32(*getLedgerEntryResult.LiveUntilLedgerSeq)
}
