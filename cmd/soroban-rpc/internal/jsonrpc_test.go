package internal

import (
	"bytes"
	"context"
	"io"
	"net"
	"net/http"
	"testing"
	"time"

	"github.com/creachadair/jrpc2"
	_ "github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
	_ "github.com/creachadair/jrpc2/handler"
	"github.com/creachadair/jrpc2/jhttp"
	_ "github.com/creachadair/jrpc2/jhttp"
	"github.com/stretchr/testify/require"
)

type TestServerHandlerWrapper struct {
	f func(http.ResponseWriter, *http.Request)
}

func (h *TestServerHandlerWrapper) ServeHTTP(res http.ResponseWriter, req *http.Request) {
	h.f(res, req)
}

func createTestServer() (serverAddr string, redirector *TestServerHandlerWrapper, shutdown context.CancelFunc) {
	ipAddr, _ := net.ResolveTCPAddr("tcp", "127.0.0.1:0")
	listener, _ := net.ListenTCP("tcp", ipAddr)
	handlerRedirector := &TestServerHandlerWrapper{}
	server := http.Server{
		Handler:           handlerRedirector,
		ReadHeaderTimeout: 10 * time.Second,
	}

	serverDown := make(chan error)
	go func() {
		serverDown <- server.Serve(listener)
	}()

	return listener.Addr().String(), handlerRedirector, func() {
		server.Shutdown(context.Background()) //nolint:errcheck
		<-serverDown
	}
}

func jrpcMethodHandler(context.Context, *jrpc2.Request) (any, error) {
	return "abc", nil
}

func TestJRPCBatching(t *testing.T) {
	handlersMap := handler.Map{}
	handlersMap["dummyMethod"] = jrpcMethodHandler
	bridge := jhttp.NewBridge(handlersMap, nil)
	serverAddress, redirector, shutdown := createTestServer()
	defer shutdown()
	redirector.f = bridge.ServeHTTP

	// make http request to the endpoint, containing two entries.
	client := http.Client{}
	request, err := http.NewRequest("POST", "http://"+serverAddress, bytes.NewBufferString(`[{"jsonrpc": "2.0", "method": "dummyMethod", "id": "1"}, {"jsonrpc": "2.0", "method": "dummyMethod", "id": "2"}]`))
	require.NoError(t, err)
	request.Header["Content-Type"] = []string{"application/json"}
	response, err := client.Do(request)
	require.NoError(t, err)
	require.Equal(t, http.StatusOK, response.StatusCode)
	responseBody, err := io.ReadAll(response.Body)
	require.NoError(t, err)
	require.Equal(t, `[{"jsonrpc":"2.0","id":"1","result":"abc"},{"jsonrpc":"2.0","id":"2","result":"abc"}]`, string(responseBody))

	t.Skip("Skipping the rest of the test, since it would fail due to jrpc2 specs misaligment. ")
	// repeat the request, this time with only a single item in the array.
	request, err = http.NewRequest("POST", "http://"+serverAddress, bytes.NewBufferString(`[{"jsonrpc": "2.0", "method": "dummyMethod", "id": "3"}]`))
	require.NoError(t, err)
	request.Header["Content-Type"] = []string{"application/json"}
	response, err = client.Do(request)
	require.NoError(t, err)
	require.Equal(t, http.StatusOK, response.StatusCode)
	responseBody, err = io.ReadAll(response.Body)
	require.NoError(t, err)
	require.Equal(t, `[{"jsonrpc":"2.0","id":"3","result":"abc"}]`, string(responseBody))

}
