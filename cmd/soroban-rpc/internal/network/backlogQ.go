package network

import (
	"context"
	"net/http"
	"sync/atomic"

	"github.com/creachadair/jrpc2"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/support/errors"
	"github.com/stellar/go/support/log"
)

type backlogQLimiter struct {
	limit        uint64
	pending      uint64
	gauge        prometheus.Gauge
	limitReached uint64
	logger       *log.Entry
}

type backlogHttpQLimiter struct {
	httpDownstreamHandler http.Handler
	backlogQLimiter
}

func MakeHttpBacklogQueueLimiter(downstream http.Handler, guage prometheus.Gauge, limit uint64, logger *log.Entry) *backlogHttpQLimiter {
	return &backlogHttpQLimiter{
		httpDownstreamHandler: downstream,
		backlogQLimiter: backlogQLimiter{
			limit:  limit,
			gauge:  guage,
			logger: logger,
		},
	}
}

type backlogJrpcQLimiter struct {
	jrpcDownstreamHandler jrpc2.Handler
	backlogQLimiter
}

func MakeJrpcBacklogQueueLimiter(downstream jrpc2.Handler, guage prometheus.Gauge, limit uint64, logger *log.Entry) *backlogJrpcQLimiter {
	return &backlogJrpcQLimiter{
		jrpcDownstreamHandler: downstream,
		backlogQLimiter: backlogQLimiter{
			limit:  limit,
			gauge:  guage,
			logger: logger,
		},
	}
}

func (q *backlogHttpQLimiter) ServeHTTP(res http.ResponseWriter, req *http.Request) {
	if newPending := atomic.AddUint64(&q.pending, 1); newPending > q.limit {
		// we've reached our queue limit - let the caller know we're too busy.
		atomic.AddUint64(&q.pending, ^uint64(0))
		res.WriteHeader(http.StatusTooManyRequests)
		if atomic.CompareAndSwapUint64(&q.limitReached, 0, 1) {
			// if the limit was reached, log a message.
			if q.logger != nil {
				q.logger.Infof("Backlog queue limiter reached the queue limit of %d executing concurrent http requests.", q.limit)
			}
		}
		return
	} else {
		if q.gauge != nil {
			q.gauge.Set(float64(newPending))
		}
	}
	defer func() {
		newPending := atomic.AddUint64(&q.pending, ^uint64(0))
		if q.gauge != nil {
			q.gauge.Set(float64(newPending))
		}
		atomic.StoreUint64(&q.limitReached, 0)
	}()

	q.httpDownstreamHandler.ServeHTTP(res, req)
}

func (q *backlogJrpcQLimiter) Handle(ctx context.Context, req *jrpc2.Request) (interface{}, error) {
	if newPending := atomic.AddUint64(&q.pending, 1); newPending > q.limit {
		// we've reached our queue limit - let the caller know we're too busy.
		atomic.AddUint64(&q.pending, ^uint64(0))
		if atomic.CompareAndSwapUint64(&q.limitReached, 0, 1) {
			// if the limit was reached, log a message.
			if q.logger != nil {
				q.logger.Infof("Backlog queue limiter reached the queue limit of %d executing concurrent rpc %s requests.", q.limit, req.Method())
			}
		}
		return nil, errors.Errorf("rpc queue for %s surpassed queue limit of %d requests", req.Method(), q.limit)
	} else {
		if q.gauge != nil {
			q.gauge.Set(float64(newPending))
		}
	}
	defer func() {
		newPending := atomic.AddUint64(&q.pending, ^uint64(0))
		if q.gauge != nil {
			q.gauge.Set(float64(newPending))
		}
		atomic.StoreUint64(&q.limitReached, 0)
	}()

	return q.jrpcDownstreamHandler.Handle(ctx, req)
}
