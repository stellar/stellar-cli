package test

import (
	"context"
	"crypto/sha256"
	"testing"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/stellar/go/keypair"
	"github.com/stellar/go/txnbuild"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

func TestEvictTemporaryLedgerEntries(t *testing.T) {
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
	invokeIncTemporaryEntryParams := txnbuild.TransactionParams{
		SourceAccount:        &account,
		IncrementSequenceNum: true,
		Operations: []txnbuild.Operation{
			createInvokeHostOperation(
				address,
				contractID,
				"inc_tmp",
			),
		},
		BaseFee: txnbuild.MinBaseFee,
		Preconditions: txnbuild.Preconditions{
			TimeBounds: txnbuild.NewInfiniteTimeout(),
		},
	}
	params = preflightTransactionParams(t, client, invokeIncTemporaryEntryParams)
	tx, err = txnbuild.NewTransaction(params)
	assert.NoError(t, err)
	sendSuccessfulTransaction(t, client, sourceAccount, tx)

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
		},
	}

	// make sure the ledger entry exists and so does the expiration entry counterpart

	keyB64, err := xdr.MarshalBase64(key)
	require.NoError(t, err)

	getLedgerEntryRequest := methods.GetLedgerEntryRequest{
		Key: keyB64,
	}
	var getLedgerEntryResult methods.GetLedgerEntryResponse
	err = client.CallResult(context.Background(), "getLedgerEntry", getLedgerEntryRequest, &getLedgerEntryResult)
	require.NoError(t, err)

	binKey, err := key.MarshalBinary()
	assert.NoError(t, err)

	expirationKey := xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeExpiration,
		Expiration: &xdr.LedgerKeyExpiration{
			KeyHash: sha256.Sum256(binKey),
		},
	}

	keyB64, err = xdr.MarshalBase64(expirationKey)
	require.NoError(t, err)
	getExpirationLedgerEntryRequest := methods.GetLedgerEntryRequest{
		Key: keyB64,
	}

	err = client.CallResult(context.Background(), "getLedgerEntry", getExpirationLedgerEntryRequest, &getLedgerEntryResult)
	assert.NoError(t, err)

	// Wait until the entry gets evicted
	evicted := false
	for i := 0; i < 5000; i++ {
		err = client.CallResult(context.Background(), "getLedgerEntry", getLedgerEntryRequest, &getLedgerEntryResult)
		if err != nil {
			evicted = true
			t.Logf("ledger entry evicted")
			break
		}
		t.Log("waiting for ledger entry to get evicted")
		time.Sleep(time.Second)
	}

	require.True(t, evicted)

	// Make sure that the expiration ledger entry was also evicted
	err = client.CallResult(context.Background(), "getLedgerEntry", getExpirationLedgerEntryRequest, &getLedgerEntryResult)
	assert.Error(t, err)
}
