package test

import (
	"context"
	"fmt"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stretchr/testify/assert"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
)

func TestGetNetworkSucceeds(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	request := methods.GetNetworkRequest{}

	var result methods.GetNetworkResponse
	err := client.CallResult(context.Background(), "getNetwork", request, &result)
	assert.NoError(t, err)
	assert.Equal(t, result.FriendbotURL, "?friendbot?")
	assert.Equal(t, result.Passphrase, StandaloneNetworkPassphrase)
	assert.Equal(t, result.ProtocolVersion, fmt.Sprint(StellarCoreProtocolVersion))
}

func TestGetNetworkCoreClientError(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	client := jrpc2.NewClient(ch, nil)

	request := methods.GetNetworkRequest{}

	var result methods.GetNetworkResponse
	err := client.CallResult(context.Background(), "getNetwork", request, &result)
	assert.EqualError(t, err, "some error here")
}
