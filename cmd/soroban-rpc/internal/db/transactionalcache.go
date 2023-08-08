package db

type transactionalCache struct {
	entries map[string]string
}

func newTransactionalCache() transactionalCache {
	return transactionalCache{entries: map[string]string{}}
}

func (c transactionalCache) newReadTx() transactionalCacheReadTx {
	ret := make(transactionalCacheReadTx, len(c.entries))
	for k, v := range c.entries {
		ret[k] = v
	}
	return ret
}

func (c transactionalCache) newWriteTx(estimatedWriteCount int) transactionalCacheWriteTx {
	return transactionalCacheWriteTx{
		pendingUpdates: make(map[string]*string, estimatedWriteCount),
		parent:         &c,
	}
}

type transactionalCacheReadTx map[string]string

func (r transactionalCacheReadTx) get(key string) (string, bool) {
	val, ok := r[key]
	return val, ok
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
