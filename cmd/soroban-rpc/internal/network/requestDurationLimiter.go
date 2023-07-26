package network

import (
	"context"
	"net/http"
	"time"

	"github.com/stellar/go/support/log"
)

type requestDurationLimiter struct {
	warningThreshold time.Duration
	limitThreshold   time.Duration
	logger           *log.Entry
}

type httpRequestDurationLimiter struct {
	httpDownstreamHandler http.Handler
	requestDurationLimiter
}

func MakeHTTPRequestDurationLimiter(
	downstream http.Handler,
	warningThreshold time.Duration,
	limitThreshold time.Duration,
	logger *log.Entry) *httpRequestDurationLimiter {
	return &httpRequestDurationLimiter{
		httpDownstreamHandler: downstream,
		requestDurationLimiter: requestDurationLimiter{
			warningThreshold: warningThreshold,
			limitThreshold:   limitThreshold,
			logger:           logger,
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

func (w *bufferedResponseWriter) WriteOut(rw http.ResponseWriter) {
	// update the headers map.
	headers := rw.Header()
	for k := range headers {
		delete(headers, k)
	}
	for k, v := range w.header {
		headers[k] = v
	}
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

	for {
		select {
		case <-warningCh:
			// warn
			if q.logger != nil {
				q.logger.Infof("Request processing for %s exceed warning threshold of %v", req.URL.Path, q.warningThreshold)
			}
		case <-limitCh:
			// limit
			requestCtxCancel()
			if q.logger != nil {
				q.logger.Infof("Request processing for %s exceed limiting threshold of %v", req.URL.Path, q.limitThreshold)
			}
			res.WriteHeader(http.StatusGatewayTimeout)
			return
		case <-requestCompleted:
			responseBuffer.WriteOut(res)
			return
		}
	}
}
