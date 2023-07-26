package network

import (
	"context"
	"io"
	"net"
	"net/http"
	"testing"
	"time"

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
		Handler: handlerRedirector,
	}

	serverDown := make(chan interface{})
	go func() {
		server.Serve(listener)
		close(serverDown)
	}()

	return listener.Addr().String(), handlerRedirector, func() {
		server.Shutdown(context.Background())
		<-serverDown
	}

}
func TestRequestDurationLimiter_Limiting(t *testing.T) {
	addr, redirector, shutdown := createTestServer()
	longExecutingHandler := &TestServerHandlerWrapper{
		f: func(res http.ResponseWriter, req *http.Request) {
			select {
			case <-req.Context().Done():
				return
			case <-time.After(time.Second * 10):
			}
			res.Write([]byte{1, 2, 3})
		},
	}
	redirector.f = MakeHTTPRequestDurationLimiter(
		longExecutingHandler,
		time.Second/20,
		time.Second/10,
		nil).ServeHTTP

	client := http.Client{}
	resp, err := client.Get("http://" + addr + "/")
	require.NoError(t, err)
	bytes, err := io.ReadAll(resp.Body)
	require.NoError(t, err)
	require.Equal(t, []byte{}, bytes)
	require.Equal(t, resp.StatusCode, http.StatusGatewayTimeout)
	shutdown()
}

func TestRequestDurationLimiter_NoLimiting(t *testing.T) {
	addr, redirector, shutdown := createTestServer()
	longExecutingHandler := &TestServerHandlerWrapper{
		f: func(res http.ResponseWriter, req *http.Request) {
			select {
			case <-req.Context().Done():
				return
			case <-time.After(time.Second / 10):
			}
			res.Write([]byte{1, 2, 3})
		},
	}
	redirector.f = MakeHTTPRequestDurationLimiter(
		longExecutingHandler,
		time.Second*5,
		time.Second*10,
		nil).ServeHTTP

	client := http.Client{}
	resp, err := client.Get("http://" + addr + "/")
	require.NoError(t, err)
	bytes, err := io.ReadAll(resp.Body)
	require.NoError(t, err)
	require.Equal(t, []byte{1, 2, 3}, bytes)
	require.Equal(t, resp.StatusCode, http.StatusOK)
	shutdown()
}
