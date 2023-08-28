package network

import (
	"context"
	"io"
	"net"
	"net/http"
	"testing"
	"time"

	"github.com/stretchr/testify/require"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
	"github.com/creachadair/jrpc2/jhttp"
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

func TestHTTPRequestDurationLimiter_Limiting(t *testing.T) {
	addr, redirector, shutdown := createTestServer()
	longExecutingHandler := &TestServerHandlerWrapper{
		f: func(res http.ResponseWriter, req *http.Request) {
			select {
			case <-req.Context().Done():
				return
			case <-time.After(time.Second * 10):
			}
			n, err := res.Write([]byte{1, 2, 3})
			require.Equal(t, 3, n)
			require.Nil(t, err)
		},
	}
	warningCounter := TestingCounter{}
	limitCounter := TestingCounter{}
	logCounter := makeTestLogCounter()
	redirector.f = MakeHTTPRequestDurationLimiter(
		longExecutingHandler,
		time.Second/20,
		time.Second/10,
		&warningCounter,
		&limitCounter,
		logCounter.Entry()).ServeHTTP

	client := http.Client{}
	req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, "http://"+addr+"/", nil)
	require.NoError(t, err)
	resp, err := client.Do(req)
	require.NoError(t, err)
	bytes, err := io.ReadAll(resp.Body)
	require.NoError(t, resp.Body.Close())
	require.NoError(t, err)
	require.Equal(t, []byte{}, bytes)
	require.Equal(t, resp.StatusCode, http.StatusGatewayTimeout)
	require.Zero(t, warningCounter.count)
	require.Equal(t, int64(1), limitCounter.count)
	require.Equal(t, [7]int{0, 0, 0, 0, 1, 0, 0}, logCounter.writtenLogEntries)
	shutdown()
}

func TestHTTPRequestDurationLimiter_NoLimiting(t *testing.T) {
	addr, redirector, shutdown := createTestServer()
	longExecutingHandler := &TestServerHandlerWrapper{
		f: func(res http.ResponseWriter, req *http.Request) {
			select {
			case <-req.Context().Done():
				return
			case <-time.After(time.Second / 10):
			}
			n, err := res.Write([]byte{1, 2, 3})
			require.Equal(t, 3, n)
			require.Nil(t, err)
		},
	}
	warningCounter := TestingCounter{}
	limitCounter := TestingCounter{}
	logCounter := makeTestLogCounter()
	redirector.f = MakeHTTPRequestDurationLimiter(
		longExecutingHandler,
		time.Second*5,
		time.Second*10,
		&warningCounter,
		&limitCounter,
		logCounter.Entry()).ServeHTTP

	client := http.Client{}
	req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, "http://"+addr+"/", nil)
	require.NoError(t, err)
	resp, err := client.Do(req)
	require.NoError(t, err)
	bytes, err := io.ReadAll(resp.Body)
	require.NoError(t, resp.Body.Close())
	require.NoError(t, err)
	require.Equal(t, []byte{1, 2, 3}, bytes)
	require.Equal(t, resp.StatusCode, http.StatusOK)
	require.Zero(t, warningCounter.count)
	require.Zero(t, limitCounter.count)
	require.Equal(t, [7]int{0, 0, 0, 0, 0, 0, 0}, logCounter.writtenLogEntries)
	shutdown()
}

func TestHTTPRequestDurationLimiter_NoLimiting_Warn(t *testing.T) {
	addr, redirector, shutdown := createTestServer()
	longExecutingHandler := &TestServerHandlerWrapper{
		f: func(res http.ResponseWriter, req *http.Request) {
			select {
			case <-req.Context().Done():
				return
			case <-time.After(time.Second / 5):
			}
			n, err := res.Write([]byte{1, 2, 3})
			require.Equal(t, 3, n)
			require.Nil(t, err)
		},
	}
	warningCounter := TestingCounter{}
	limitCounter := TestingCounter{}
	logCounter := makeTestLogCounter()
	redirector.f = MakeHTTPRequestDurationLimiter(
		longExecutingHandler,
		time.Second/10,
		time.Second*10,
		&warningCounter,
		&limitCounter,
		logCounter.Entry()).ServeHTTP

	client := http.Client{}
	req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, "http://"+addr+"/", nil)
	require.NoError(t, err)
	resp, err := client.Do(req)
	require.NoError(t, err)
	bytes, err := io.ReadAll(resp.Body)
	require.NoError(t, resp.Body.Close())
	require.NoError(t, err)
	require.Equal(t, []byte{1, 2, 3}, bytes)
	require.Equal(t, resp.StatusCode, http.StatusOK)
	require.Equal(t, int64(1), warningCounter.count)
	require.Zero(t, limitCounter.count)
	require.Equal(t, [7]int{0, 0, 0, 0, 1, 0, 0}, logCounter.writtenLogEntries)
	shutdown()
}

type JRPCHandlerFunc func(ctx context.Context, r *jrpc2.Request) (interface{}, error)

func bindRPCHoist(redirector *TestServerHandlerWrapper) *JRPCHandlerFunc {
	var hoistFunction JRPCHandlerFunc

	bridgeMap := handler.Map{
		"method": handler.New(func(ctx context.Context, r *jrpc2.Request) (interface{}, error) {
			return hoistFunction(ctx, r)
		}),
	}

	redirector.f = jhttp.NewBridge(bridgeMap, &jhttp.BridgeOptions{}).ServeHTTP
	return &hoistFunction
}

func TestJRPCRequestDurationLimiter_Limiting(t *testing.T) {
	addr, redirector, shutdown := createTestServer()
	hoistFunction := bindRPCHoist(redirector)

	longExecutingHandler := handler.New(func(ctx context.Context, r *jrpc2.Request) (interface{}, error) {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(time.Second * 10):
		}
		return "", nil
	})

	warningCounter := TestingCounter{}
	limitCounter := TestingCounter{}
	logCounter := makeTestLogCounter()
	*hoistFunction = MakeJrpcRequestDurationLimiter(
		longExecutingHandler,
		time.Second/20,
		time.Second/10,
		&warningCounter,
		&limitCounter,
		logCounter.Entry()).Handle

	ch := jhttp.NewChannel("http://"+addr+"/", nil)
	client := jrpc2.NewClient(ch, nil)

	var res interface{}
	req := struct {
		i int
	}{1}
	err := client.CallResult(context.Background(), "method", req, &res)
	require.NotNil(t, err)
	jrpcError, ok := err.(*jrpc2.Error)
	require.True(t, ok)
	require.Equal(t, ErrRequestExceededProcessingLimitThreshold.Code, jrpcError.Code)
	require.Equal(t, nil, res)
	require.Zero(t, warningCounter.count)
	require.Equal(t, int64(1), limitCounter.count)
	require.Equal(t, [7]int{0, 0, 0, 0, 1, 0, 0}, logCounter.writtenLogEntries)
	shutdown()
}

func TestJRPCRequestDurationLimiter_NoLimiting(t *testing.T) {
	addr, redirector, shutdown := createTestServer()
	hoistFunction := bindRPCHoist(redirector)

	returnString := "ok"
	longExecutingHandler := handler.New(func(ctx context.Context, r *jrpc2.Request) (interface{}, error) {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(time.Second / 10):
		}
		return returnString, nil
	})

	warningCounter := TestingCounter{}
	limitCounter := TestingCounter{}
	logCounter := makeTestLogCounter()
	*hoistFunction = MakeJrpcRequestDurationLimiter(
		longExecutingHandler,
		time.Second*5,
		time.Second*10,
		&warningCounter,
		&limitCounter,
		logCounter.Entry()).Handle

	ch := jhttp.NewChannel("http://"+addr+"/", nil)
	client := jrpc2.NewClient(ch, nil)

	var res interface{}
	req := struct {
		i int
	}{1}
	err := client.CallResult(context.Background(), "method", req, &res)
	require.Nil(t, err)
	require.Equal(t, returnString, res)
	require.Zero(t, warningCounter.count)
	require.Zero(t, limitCounter.count)
	require.Equal(t, [7]int{0, 0, 0, 0, 0, 0, 0}, logCounter.writtenLogEntries)
	shutdown()
}

func TestJRPCRequestDurationLimiter_NoLimiting_Warn(t *testing.T) {
	addr, redirector, shutdown := createTestServer()
	hoistFunction := bindRPCHoist(redirector)

	returnString := "ok"
	longExecutingHandler := handler.New(func(ctx context.Context, r *jrpc2.Request) (interface{}, error) {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(time.Second / 5):
		}
		return returnString, nil
	})

	warningCounter := TestingCounter{}
	limitCounter := TestingCounter{}
	logCounter := makeTestLogCounter()
	*hoistFunction = MakeJrpcRequestDurationLimiter(
		longExecutingHandler,
		time.Second/10,
		time.Second*10,
		&warningCounter,
		&limitCounter,
		logCounter.Entry()).Handle

	ch := jhttp.NewChannel("http://"+addr+"/", nil)
	client := jrpc2.NewClient(ch, nil)

	var res interface{}
	req := struct {
		i int
	}{1}
	err := client.CallResult(context.Background(), "method", req, &res)
	require.Nil(t, err)
	require.Equal(t, returnString, res)
	require.Equal(t, int64(1), warningCounter.count)
	require.Zero(t, limitCounter.count)
	require.Equal(t, [7]int{0, 0, 0, 0, 1, 0, 0}, logCounter.writtenLogEntries)
	shutdown()
}

func TestHTTPRequestDurationLimiter_Panicing(t *testing.T) {
	addr, redirector, shutdown := createTestServer()
	longExecutingHandler := &TestServerHandlerWrapper{
		f: func(res http.ResponseWriter, req *http.Request) {
			var panicWrite *int
			*panicWrite = 1
		},
	}

	logCounter := makeTestLogCounter()
	redirector.f = MakeHTTPRequestDurationLimiter(
		longExecutingHandler,
		time.Second*10,
		time.Second*10,
		nil,
		nil,
		logCounter.Entry()).ServeHTTP

	client := http.Client{}
	req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, "http://"+addr+"/", nil)
	require.NoError(t, err)
	resp, err := client.Do(req)
	require.NoError(t, err)
	bytes, err := io.ReadAll(resp.Body)
	require.NoError(t, err)
	require.NoError(t, resp.Body.Close())
	require.Equal(t, http.StatusInternalServerError, resp.StatusCode)
	require.Equal(t, []byte{}, bytes)
	require.Equal(t, [7]int{0, 0, 0, 7, 0, 0, 0}, logCounter.writtenLogEntries)
	shutdown()
}
