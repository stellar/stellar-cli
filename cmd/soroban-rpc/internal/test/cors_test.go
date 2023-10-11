package test

import (
	"bytes"
	"io"
	"net/http"
	"testing"

	"github.com/stretchr/testify/require"
)

// TestCORS ensures that we receive the correct CORS headers as a response to an HTTP request.
// Specifically, when we include an Origin header in the request, a soroban-rpc should response
// with a corresponding Access-Control-Allow-Origin.
func TestCORS(t *testing.T) {
	test := NewTest(t)

	request, err := http.NewRequest("POST", test.sorobanRPCURL(), bytes.NewBufferString("{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"getHealth\"}"))
	require.NoError(t, err)
	request.Header.Set("Content-Type", "application/json")
	origin := "testorigin.com"
	request.Header.Set("Origin", origin)

	var client http.Client
	response, err := client.Do(request)
	require.NoError(t, err)
	_, err = io.ReadAll(response.Body)
	require.NoError(t, err)

	accessControl := response.Header.Get("Access-Control-Allow-Origin")
	require.Equal(t, origin, accessControl)
}
