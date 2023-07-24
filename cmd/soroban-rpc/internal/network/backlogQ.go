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
	httpDownstreamHandler http.Handler
	jrpcDownstreamHandler jrpc2.Handler
	limit                 uint64
	pending               uint64
	guage                 prometheus.Gauge
	limitReached          uint64
	logger                *log.Entry
}

func MakeHttpBacklogQueueLimiter(downstream http.Handler, guage prometheus.Gauge, limit uint64, logger *log.Entry) *backlogQLimiter {
	return &backlogQLimiter{
		httpDownstreamHandler: downstream,
		limit:                 limit,
		guage:                 guage,
		logger:                logger,
	}
}

func MakeJrpcBacklogQueueLimiter(downstream jrpc2.Handler, guage prometheus.Gauge, limit uint64, logger *log.Entry) *backlogQLimiter {
	return &backlogQLimiter{
		jrpcDownstreamHandler: downstream,
		limit:                 limit,
		guage:                 guage,
		logger:                logger,
	}
}

func (q *backlogQLimiter) ServeHTTP(res http.ResponseWriter, req *http.Request) {
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
		if q.guage != nil {
			q.guage.Set(float64(newPending))
		}
	}
	defer func() {
		newPending := atomic.AddUint64(&q.pending, ^uint64(0))
		if q.guage != nil {
			q.guage.Set(float64(newPending))
		}
		atomic.StoreUint64(&q.limitReached, 0)
	}()

	q.httpDownstreamHandler.ServeHTTP(res, req)
}

func (q *backlogQLimiter) Handle(ctx context.Context, req *jrpc2.Request) (interface{}, error) {
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
		if q.guage != nil {
			q.guage.Set(float64(newPending))
		}
	}
	defer func() {
		newPending := atomic.AddUint64(&q.pending, ^uint64(0))
		if q.guage != nil {
			q.guage.Set(float64(newPending))
		}
		atomic.StoreUint64(&q.limitReached, 0)
	}()

	return q.jrpcDownstreamHandler.Handle(ctx, req)
}
