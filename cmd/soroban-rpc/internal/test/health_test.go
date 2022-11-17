package test

import (
	"context"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal"
	"github.com/stretchr/testify/assert"
)

func TestHealth(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.server.URL, nil)
	cli := jrpc2.NewClient(ch, nil)

	var result internal.HealthCheckResult
	if err := cli.CallResult(context.Background(), "getHealth", nil, &result); err != nil {
		t.Fatalf("rpc call failed: %v", err)
	}
	assert.Equal(t, internal.HealthCheckResult{Status: "healthy"}, result)
}
