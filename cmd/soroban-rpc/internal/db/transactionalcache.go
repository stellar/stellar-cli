package db

type transactionalCache struct {
	entries map[string]string
}

func newTransactionalCache() transactionalCache {
	return transactionalCache{entries: map[string]string{}}
}

func (c transactionalCache) newReadTx() transactionalCacheReadTx {
	entries := make(map[string]*string, len(c.entries))
	for k, v := range c.entries {
		localV := v
		entries[k] = &localV
	}
	return transactionalCacheReadTx{entries: entries}
}

func (c transactionalCache) newWriteTx(estimatedWriteCount int) transactionalCacheWriteTx {
	return transactionalCacheWriteTx{
		pendingUpdates: make(map[string]*string, estimatedWriteCount),
		parent:         &c,
	}
}

// nil indicates not present in the underlying storage
type transactionalCacheReadTx struct {
	entries map[string]*string
}

// nil indicates not present in the underlying storage
func (r transactionalCacheReadTx) get(key string) (*string, bool) {
	val, ok := r.entries[key]
	return val, ok
}

// nil indicates not present in the underlying storage
func (r transactionalCacheReadTx) upsert(key string, value *string) {
	r.entries[key] = value
}

type transactionalCacheWriteTx struct {
	// nil indicates deletion
	pendingUpdates map[string]*string
	parent         *transactionalCache
}

func (w transactionalCacheWriteTx) upsert(key, val string) {
	w.pendingUpdates[key] = &val
}

func (w transactionalCacheWriteTx) delete(key string) {
	w.pendingUpdates[key] = nil
}

func (w transactionalCacheWriteTx) commit() {
	for key, newValue := range w.pendingUpdates {
		if newValue == nil {
			delete(w.parent.entries, key)
		} else {
			w.parent.entries[key] = *newValue
		}
	}
}
