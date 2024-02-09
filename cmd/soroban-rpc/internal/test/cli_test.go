package test

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path"
	"runtime"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/google/shlex"
	"github.com/stellar/go/keypair"
	proto "github.com/stellar/go/protocols/stellarcore"
	"github.com/stellar/go/strkey"
	"github.com/stellar/go/txnbuild"
	"github.com/stellar/go/xdr"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"gotest.tools/v3/icmd"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

var (
	testSalt = sha256.Sum256([]byte("a1"))
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

func getCounterLedgerKey(contractID [32]byte) xdr.LedgerKey {
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
	return key
}

func waitUntilLedgerEntryTTL(t *testing.T, client *jrpc2.Client, ledgerKey xdr.LedgerKey) {
	keyB64, err := xdr.MarshalBase64(ledgerKey)
	require.NoError(t, err)
	request := methods.GetLedgerEntriesRequest{
		Keys: []string{keyB64},
	}
	ttled := false
	for i := 0; i < 50; i++ {
		var result methods.GetLedgerEntriesResponse
		var entry xdr.LedgerEntryData
		err := client.CallResult(context.Background(), "getLedgerEntries", request, &result)
		require.NoError(t, err)
		require.NotEmpty(t, result.Entries)
		require.NoError(t, xdr.SafeUnmarshalBase64(result.Entries[0].XDR, &entry))
		require.NotEqual(t, xdr.LedgerEntryTypeTtl, entry.Type)
		liveUntilLedgerSeq := xdr.Uint32(*result.Entries[0].LiveUntilLedgerSeq)
		// See https://soroban.stellar.org/docs/fundamentals-and-concepts/state-expiration#expiration-ledger
		currentLedger := result.LatestLedger + 1
		if xdr.Uint32(currentLedger) > liveUntilLedgerSeq {
			ttled = true
			t.Logf("ledger entry ttl'ed")
			break
		}
		t.Log("waiting for ledger entry to ttl at ledger", liveUntilLedgerSeq)
		time.Sleep(time.Second)
	}
	require.True(t, ttled)
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
		if txMeta.V == 3 && txMeta.V3.SorobanMeta != nil {
			if len(txMeta.V3.SorobanMeta.Events) > 0 {
				fmt.Println("Contract events:")
				for i, e := range txMeta.V3.SorobanMeta.Events {
					fmt.Printf("  %d: %s\n", i, e)
				}
			}

			if len(txMeta.V3.SorobanMeta.DiagnosticEvents) > 0 {
				fmt.Println("Diagnostic events:")
				for i, d := range txMeta.V3.SorobanMeta.DiagnosticEvents {
					fmt.Printf("  %d: %s\n", i, d)
				}
			}
		}
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

func getHelloWorldContract(t *testing.T) []byte {
	_, filename, _, _ := runtime.Caller(0)
	testDirName := path.Dir(filename)
	contractFile := path.Join(testDirName, helloWorldContractPath)
	ret, err := os.ReadFile(contractFile)
	if err != nil {
		t.Fatalf("unable to read test_hello_world.wasm (%v) please run `make build-test-wasms` at the project root directory", err)
	}
	return ret
}
