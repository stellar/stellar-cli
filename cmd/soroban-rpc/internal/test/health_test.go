package test

import (
	"context"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/jhttp"
	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/methods"
	"github.com/stretchr/testify/assert"
)

func TestHealth(t *testing.T) {
	test := NewTest(t)

	ch := jhttp.NewChannel(test.sorobanRPCURL(), nil)
	client := jrpc2.NewClient(ch, nil)

	var result methods.HealthCheckResult
	if err := client.CallResult(context.Background(), "getHealth", nil, &result); err != nil {
		t.Fatalf("rpc call failed: %v", err)
	}
	assert.Equal(t, methods.HealthCheckResult{Status: "healthy"}, result)
}
