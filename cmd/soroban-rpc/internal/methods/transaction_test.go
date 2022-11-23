package methods

import (
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestDeleteExpiredTransaction(t *testing.T) {
	ttl := time.Minute
	proxy := NewTransactionProxy(
		nil,
		10,
		10,
		"",
		ttl,
	)
	pending := transactionResult{
		pending: true,
	}
	proxy.results["a"] = pending
	proxy.results["b"] = pending
	t.Run("ignores pending", func(t *testing.T) {
		proxy.deleteExpiredEntries(time.Now())
		assert.Len(t, proxy.results, 2)

		assert.Equal(t, pending, proxy.results["a"])
		assert.Equal(t, pending, proxy.results["b"])
	})

	proxy.results = map[string]transactionResult{}
	proxy.results["a"] = transactionResult{
		pending: false,
	}
	proxy.results["b"] = transactionResult{
		pending:   false,
		timestamp: time.Now().Add(-time.Hour),
	}
	notYetExpired := transactionResult{
		pending:   false,
		timestamp: time.Now().Add(-time.Second),
	}
	proxy.results["c"] = notYetExpired
	proxy.results["d"] = pending
	t.Run("ignores pending", func(t *testing.T) {
		proxy.deleteExpiredEntries(time.Now())
		assert.Len(t, proxy.results, 2)

		assert.Equal(t, notYetExpired, proxy.results["c"])
		assert.Equal(t, pending, proxy.results["d"])
	})

}
