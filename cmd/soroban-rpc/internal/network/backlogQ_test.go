package network

import (
	"context"
	"math/rand"
	"net/http"
	"sync"
	"sync/atomic"
	"testing"

	"github.com/creachadair/jrpc2"
	"github.com/stretchr/testify/require"
)

type TestingHandlerWrapper struct {
	f func(http.ResponseWriter, *http.Request)
}

func (t *TestingHandlerWrapper) ServeHTTP(res http.ResponseWriter, req *http.Request) {
	t.f(res, req)
}

type TestingJrpcHandlerWrapper struct {
	f func(context.Context, *jrpc2.Request) (interface{}, error)
}

func (t *TestingJrpcHandlerWrapper) Handle(ctx context.Context, req *jrpc2.Request) (interface{}, error) {
	return t.f(ctx, req)
}

// The goal of the TestBacklogQueueLimiter_HttpNonBlocking is to try
// and enquque load against the queue limiter, without hitting the
// limit. All request should pass through.
func TestBacklogQueueLimiter_HttpNonBlocking(t *testing.T) {
	var sum uint64
	var wg sync.WaitGroup
	requestsSizeLimit := uint64(1000)
	adding := &TestingHandlerWrapper{f: func(res http.ResponseWriter, req *http.Request) {
		atomic.AddUint64(&sum, 1)
	}}

	logCounter := makeTestLogCounter()
	testGauge := &TestingGauge{}
	limiter := MakeHTTPBacklogQueueLimiter(adding, testGauge, requestsSizeLimit, logCounter.Entry())
	for i := 1; i < 50; i++ {
		n := rand.Int63n(int64(requestsSizeLimit)) //nolint:gosec
		require.Zero(t, int(testGauge.count))
		wg.Add(int(n))
		for k := n; k > 0; k-- {
			go func() {
				limiter.ServeHTTP(nil, nil)
				wg.Done()
			}()
		}
		wg.Wait()
		require.Equal(t, uint64(n), sum)
		require.Zero(t, int(testGauge.count))
		sum = 0
	}
	require.Equal(t, [7]int{0, 0, 0, 0, 0, 0, 0}, logCounter.writtenLogEntries)
}

// The goal of the TestBacklogQueueLimiter_HttpNonBlocking is to try
// and enquque load against the queue limiter, without hitting the
// limit. All request should pass through.
func TestBacklogQueueLimiter_JrpcNonBlocking(t *testing.T) {
	var sum uint64
	var wg sync.WaitGroup
	requestsSizeLimit := uint64(1000)
	adding := &TestingJrpcHandlerWrapper{f: func(context.Context, *jrpc2.Request) (interface{}, error) {
		atomic.AddUint64(&sum, 1)
		return nil, nil
	}}
	logCounter := makeTestLogCounter()
	testGauge := &TestingGauge{}
	limiter := MakeJrpcBacklogQueueLimiter(adding.Handle, testGauge, requestsSizeLimit, logCounter.Entry())
	for i := 1; i < 50; i++ {
		n := rand.Int63n(int64(requestsSizeLimit)) //nolint:gosec
		require.Zero(t, int(testGauge.count))
		wg.Add(int(n))
		for k := n; k > 0; k-- {
			go func() {
				_, err := limiter.Handle(context.Background(), nil)
				require.Nil(t, err)
				wg.Done()
			}()
		}
		wg.Wait()
		require.Zero(t, int(testGauge.count))
		require.Equal(t, uint64(n), sum)
		sum = 0
	}
	require.Equal(t, [7]int{0, 0, 0, 0, 0, 0, 0}, logCounter.writtenLogEntries)
}

// The goal of the TestBacklogQueueLimiter_HttpBlocking is to set
// up a queue that already reached it's limit and see that
// additional requests are being rejected. Then, unblock the queue
// and see that requests could go though.
func TestBacklogQueueLimiter_HttpBlocking(t *testing.T) {
	for _, queueSize := range []uint64{7, 50, 80} {
		blockedCh := make(chan interface{})
		var initialGroupBlocking sync.WaitGroup
		initialGroupBlocking.Add(int(queueSize) / 2)
		blockedHandlers := &TestingHandlerWrapper{f: func(res http.ResponseWriter, req *http.Request) {
			initialGroupBlocking.Done()
			<-blockedCh
		}}
		logCounter := makeTestLogCounter()
		testGauge := &TestingGauge{}
		limiter := MakeHTTPBacklogQueueLimiter(blockedHandlers, testGauge, queueSize, logCounter.Entry())
		for i := uint64(0); i < queueSize/2; i++ {
			go func() {
				limiter.ServeHTTP(nil, nil)
				initialGroupBlocking.Done()
			}()
		}

		initialGroupBlocking.Wait()
		require.Equal(t, int(queueSize)/2, int(testGauge.count))

		var secondBlockingGroupWg sync.WaitGroup
		secondBlockingGroupWg.Add(int(queueSize) - int(queueSize)/2)
		secondBlockingGroupWgCh := make(chan interface{})
		secondBlockingGroupWgHandlers := &TestingHandlerWrapper{f: func(res http.ResponseWriter, req *http.Request) {
			secondBlockingGroupWg.Done()
			<-secondBlockingGroupWgCh
		}}

		limiter.httpDownstreamHandler = secondBlockingGroupWgHandlers
		for i := queueSize / 2; i < queueSize; i++ {
			go func() {
				limiter.ServeHTTP(nil, nil)
				secondBlockingGroupWg.Done()
			}()
		}

		secondBlockingGroupWg.Wait()
		require.Equal(t, [7]int{0, 0, 0, 0, 0, 0, 0}, logCounter.writtenLogEntries)
		require.Equal(t, int(queueSize), int(testGauge.count))
		// now, try to place additional entry - which should be blocked.
		var res TestingResponseWriter
		limiter.ServeHTTP(&res, nil)
		require.Equal(t, http.StatusServiceUnavailable, res.statusCode)
		require.Equal(t, [7]int{0, 0, 0, 0, 1, 0, 0}, logCounter.writtenLogEntries)
		require.Equal(t, int(queueSize), int(testGauge.count))

		secondBlockingGroupWg.Add(int(queueSize) - int(queueSize)/2)
		// unblock the second group.
		close(secondBlockingGroupWgCh)
		secondBlockingGroupWg.Wait()
		require.Equal(t, int(queueSize)/2, int(testGauge.count))

		// see that we have no blocking
		res = TestingResponseWriter{}
		require.Equal(t, 0, res.statusCode)

		// unblock the first group.
		initialGroupBlocking.Add(int(queueSize) / 2)
		close(blockedCh)
		initialGroupBlocking.Wait()
		require.Equal(t, [7]int{0, 0, 0, 0, 1, 0, 0}, logCounter.writtenLogEntries)
		require.Zero(t, int(testGauge.count))
	}
}

// The goal of the TestBacklogQueueLimiter_JrpcBlocking is to set
// up a queue that already reached it's limit and see that
// additional requests are being rejected. Then, unblock the queue
// and see that requests could go though.
func TestBacklogQueueLimiter_JrpcBlocking(t *testing.T) {
	for _, queueSize := range []uint64{7, 50, 80} {
		blockedCh := make(chan interface{})
		var initialGroupBlocking sync.WaitGroup
		initialGroupBlocking.Add(int(queueSize) / 2)
		blockedHandlers := &TestingJrpcHandlerWrapper{f: func(context.Context, *jrpc2.Request) (interface{}, error) {
			initialGroupBlocking.Done()
			<-blockedCh
			return nil, nil
		}}
		logCounter := makeTestLogCounter()
		testGauge := &TestingGauge{}
		limiter := MakeJrpcBacklogQueueLimiter(blockedHandlers.Handle, testGauge, queueSize, logCounter.Entry())
		for i := uint64(0); i < queueSize/2; i++ {
			go func() {
				_, err := limiter.Handle(context.Background(), &jrpc2.Request{})
				require.Nil(t, err)
				initialGroupBlocking.Done()
			}()
		}
		initialGroupBlocking.Wait()
		require.Equal(t, int(queueSize)/2, int(testGauge.count))

		var secondBlockingGroupWg sync.WaitGroup
		secondBlockingGroupWg.Add(int(queueSize) - int(queueSize)/2)
		secondBlockingGroupWgCh := make(chan interface{})
		secondBlockingGroupWgHandlers := &TestingJrpcHandlerWrapper{f: func(context.Context, *jrpc2.Request) (interface{}, error) {
			secondBlockingGroupWg.Done()
			<-secondBlockingGroupWgCh
			return nil, nil
		}}

		limiter.jrpcDownstreamHandler = secondBlockingGroupWgHandlers.Handle
		for i := queueSize / 2; i < queueSize; i++ {
			go func() {
				_, err := limiter.Handle(context.Background(), &jrpc2.Request{})
				require.Nil(t, err)
				secondBlockingGroupWg.Done()
			}()
		}
		secondBlockingGroupWg.Wait()
		require.Equal(t, [7]int{0, 0, 0, 0, 0, 0, 0}, logCounter.writtenLogEntries)
		require.Equal(t, int(queueSize), int(testGauge.count))
		// now, try to place additional entry - which should be blocked.
		var res TestingResponseWriter
		_, err := limiter.Handle(context.Background(), &jrpc2.Request{})
		require.NotNil(t, err)
		require.Equal(t, [7]int{0, 0, 0, 0, 1, 0, 0}, logCounter.writtenLogEntries)

		secondBlockingGroupWg.Add(int(queueSize) - int(queueSize)/2)
		// unblock the second group.
		close(secondBlockingGroupWgCh)
		secondBlockingGroupWg.Wait()
		require.Equal(t, int(queueSize)/2, int(testGauge.count))

		// see that we have no blocking
		res = TestingResponseWriter{}
		require.Equal(t, 0, res.statusCode)

		// unblock the first group.
		initialGroupBlocking.Add(int(queueSize) / 2)
		close(blockedCh)
		initialGroupBlocking.Wait()
		require.Equal(t, [7]int{0, 0, 0, 0, 1, 0, 0}, logCounter.writtenLogEntries)
		require.Zero(t, int(testGauge.count))
	}
}
