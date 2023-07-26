package network

import (
	"context"
	"errors"
	"net/http"
	"time"

	"github.com/creachadair/jrpc2"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/support/log"
)

type requestDurationLimiter struct {
	warningThreshold time.Duration
	limitThreshold   time.Duration
	logger           *log.Entry
	warningCounter   prometheus.Counter
	limitCounter     prometheus.Counter
}

type httpRequestDurationLimiter struct {
	httpDownstreamHandler http.Handler
	requestDurationLimiter
}

func MakeHTTPRequestDurationLimiter(
	downstream http.Handler,
	warningThreshold time.Duration,
	limitThreshold time.Duration,
	warningCounter prometheus.Counter,
	limitCounter prometheus.Counter,
	logger *log.Entry) *httpRequestDurationLimiter {
	return &httpRequestDurationLimiter{
		httpDownstreamHandler: downstream,
		requestDurationLimiter: requestDurationLimiter{
			warningThreshold: warningThreshold,
			limitThreshold:   limitThreshold,
			logger:           logger,
			warningCounter:   warningCounter,
			limitCounter:     limitCounter,
		},
	}
}

type bufferedResponseWriter struct {
	header     http.Header
	buffer     []byte
	statusCode int
}

func makeBufferedResponseWriter(rw http.ResponseWriter) *bufferedResponseWriter {
	header := rw.Header()
	bw := &bufferedResponseWriter{
		header: make(http.Header, 0),
	}
	for k, v := range header {
		bw.header[k] = v
	}
	return bw
}

func (w *bufferedResponseWriter) Header() http.Header {
	return w.header
}
func (w *bufferedResponseWriter) Write(buf []byte) (int, error) {
	w.buffer = append(w.buffer, buf...)
	return len(buf), nil
}
func (w *bufferedResponseWriter) WriteHeader(statusCode int) {
	w.statusCode = statusCode
}

func (w *bufferedResponseWriter) WriteOut(ctx context.Context, rw http.ResponseWriter) {
	// update the headers map.
	headers := rw.Header()
	for k := range headers {
		delete(headers, k)
	}
	for k, v := range w.header {
		headers[k] = v
	}
	complete := make(chan interface{})
	go func() {
		if len(w.buffer) == 0 {
			if w.statusCode != 0 {
				rw.WriteHeader(w.statusCode)
			}
			return
		}
		if w.statusCode != 0 {
			rw.WriteHeader(w.statusCode)
		}
		rw.Write(w.buffer)
		close(complete)
	}()
	select {
	case <-complete:
	case <-ctx.Done():
	}
}

func (q *httpRequestDurationLimiter) ServeHTTP(res http.ResponseWriter, req *http.Request) {
	var warningCh <-chan time.Time
	if q.warningThreshold != time.Duration(0) && q.warningThreshold < q.limitThreshold {
		warningCh = time.NewTimer(q.warningThreshold).C
	}
	var limitCh <-chan time.Time
	if q.limitThreshold != time.Duration(0) {
		limitCh = time.NewTimer(q.limitThreshold).C
	}
	requestCompleted := make(chan interface{}, 1)
	requestCtx, requestCtxCancel := context.WithTimeout(req.Context(), q.limitThreshold)
	defer requestCtxCancel()
	timeLimitedRequest := req.WithContext(requestCtx)
	responseBuffer := makeBufferedResponseWriter(res)
	go func() {
		q.httpDownstreamHandler.ServeHTTP(responseBuffer, timeLimitedRequest)
		close(requestCompleted)
	}()

	warn := false
	for {
		select {
		case <-warningCh:
			// warn
			warn = true
		case <-limitCh:
			// limit
			requestCtxCancel()
			if q.limitCounter != nil {
				q.limitCounter.Inc()
			}
			if q.logger != nil {
				q.logger.Infof("Request processing for %s exceed limiting threshold of %v", req.URL.Path, q.limitThreshold)
			}
			if req.Context().Err() == nil {
				res.WriteHeader(http.StatusGatewayTimeout)
			}
			return
		case <-requestCompleted:
			if warn {
				if q.warningCounter != nil {
					q.warningCounter.Inc()
				}
				if q.logger != nil {
					q.logger.Infof("Request processing for %s exceed warning threshold of %v", req.URL.Path, q.warningThreshold)
				}
			}
			responseBuffer.WriteOut(req.Context(), res)
			return
		}
	}
}

type rpcRequestDurationLimiter struct {
	jrpcDownstreamHandler jrpc2.Handler
	requestDurationLimiter
}

func MakeRPCRequestDurationLimiter(
	downstream jrpc2.Handler,
	warningThreshold time.Duration,
	limitThreshold time.Duration,
	warningCounter prometheus.Counter,
	limitCounter prometheus.Counter,
	logger *log.Entry) *rpcRequestDurationLimiter {
	return &rpcRequestDurationLimiter{
		jrpcDownstreamHandler: downstream,
		requestDurationLimiter: requestDurationLimiter{
			warningThreshold: warningThreshold,
			limitThreshold:   limitThreshold,
			logger:           logger,
			warningCounter:   warningCounter,
			limitCounter:     limitCounter,
		},
	}
}

func (q *rpcRequestDurationLimiter) Handle(ctx context.Context, req *jrpc2.Request) (interface{}, error) {
	var warningCh <-chan time.Time
	if q.warningThreshold != time.Duration(0) && q.warningThreshold < q.limitThreshold {
		warningCh = time.NewTimer(q.warningThreshold).C
	}
	var limitCh <-chan time.Time
	if q.limitThreshold != time.Duration(0) {
		limitCh = time.NewTimer(q.limitThreshold).C
	}
	type requestResultOutput struct {
		data interface{}
		err  error
	}
	requestCompleted := make(chan requestResultOutput, 1)
	requestCtx, requestCtxCancel := context.WithTimeout(ctx, q.limitThreshold)
	defer requestCtxCancel()

	go func() {
		defer func() {
			if err := recover(); err != nil {
				q.logger.Errorf("Request for method %s resulted in an error : %v", req.Method(), err)
			}
			close(requestCompleted)
		}()
		var res requestResultOutput
		res.data, res.err = q.jrpcDownstreamHandler.Handle(requestCtx, req)
		requestCompleted <- res
	}()

	warn := false
	for {
		select {
		case <-warningCh:
			// warn
			warn = true
		case <-limitCh:
			// limit
			requestCtxCancel()
			if q.limitCounter != nil {
				q.limitCounter.Inc()
			}
			if q.logger != nil {
				q.logger.Infof("Request processing for %s exceed limiting threshold of %v", req.Method(), q.limitThreshold)
			}
			if ctxErr := ctx.Err(); ctxErr == nil {
				return nil, ErrRequestExceededProcessingLimitThreshold
			} else {
				return nil, ctxErr
			}
		case requestRes, ok := <-requestCompleted:
			if warn {
				if q.warningCounter != nil {
					q.warningCounter.Inc()
				}
				if q.logger != nil {
					q.logger.Infof("Request processing for %s exceed warning threshold of %v", req.Method(), q.warningThreshold)
				}
			}
			if ok {
				return requestRes.data, requestRes.err
			} else {
				// request panicked ?
				return nil, ErrFailToProcessDueToInternalIssue
			}
		}
	}
}

var ErrRequestExceededProcessingLimitThreshold = errors.New("request exceeded processing limit threshold")
var ErrFailToProcessDueToInternalIssue = errors.New("request failed to process due to internal issue")
