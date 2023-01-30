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
	assert.Equal(t, FriendbotURL, result.FriendbotURL)
	assert.Equal(t, StandaloneNetworkPassphrase, result.Passphrase)
	assert.Equal(t, StellarCoreProtocolVersion, result.ProtocolVersion)
}
