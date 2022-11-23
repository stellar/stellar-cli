package test

import (
	"context"
	"testing"

	"github.com/creachadair/jrpc2/code"
	"github.com/stellar/go/keypair"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stretchr/testify/assert"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

func TestAccount(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	request := methods.AccountRequest{
		Address: keypair.Master(StandaloneNetworkPassphrase).Address(),
	}
	var result methods.AccountInfo
	if err := client.CallResult(context.Background(), "getAccount", request, &result); err != nil {
		t.Fatalf("rpc call failed: %v", err)
	}
	assert.Equal(t, methods.AccountInfo{ID: request.Address, Sequence: 0}, result)

	request.Address = "invalid"
	err := client.CallResult(context.Background(), "getAccount", request, &result).(*jrpc2.Error)
	assert.Equal(t, "Bad Request", err.Message)
	assert.Equal(t, code.InvalidRequest, err.Code)
	assert.Equal(
		t,
		"{\"invalid_field\":\"account_id\",\"reason\":\"Account ID must start with `G` and contain 56 alphanum characters\"}",
		string(err.Data),
	)
}
