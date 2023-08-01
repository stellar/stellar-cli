package network

import (
	"net/http"
	"sync/atomic"

	"github.com/sirupsen/logrus"
	"github.com/stellar/go/support/log"
)

type TestingCounter struct {
	count int64
}

func (tc *TestingCounter) Inc() {
	atomic.AddInt64(&tc.count, 1)
}

type TestingGauge struct {
	count int64
}

func (tg *TestingGauge) Inc() {
	atomic.AddInt64(&tg.count, 1)
}

func (tg *TestingGauge) Dec() {
	atomic.AddInt64(&tg.count, -1)
}

type TestLogsCounter struct {
	entry             *log.Entry
	writtenLogEntries [logrus.TraceLevel + 1]int
}

func makeTestLogCounter() *TestLogsCounter {
	out := &TestLogsCounter{
		entry: log.New(),
	}
	out.entry.AddHook(out)
	out.entry.SetLevel(logrus.DebugLevel)
	return out
}
func (te *TestLogsCounter) Entry() *log.Entry {
	return te.entry
}
func (te *TestLogsCounter) Levels() []logrus.Level {
	return []logrus.Level{logrus.PanicLevel, logrus.FatalLevel, logrus.ErrorLevel, logrus.WarnLevel, logrus.InfoLevel, logrus.DebugLevel, logrus.TraceLevel}
}
func (te *TestLogsCounter) Fire(e *logrus.Entry) error {
	te.writtenLogEntries[e.Level]++
	return nil
}

type TestingResponseWriter struct {
	statusCode int
}

func (t *TestingResponseWriter) Header() http.Header {
	return http.Header{}
}
func (t *TestingResponseWriter) Write([]byte) (int, error) {
	return 0, nil
}

func (t *TestingResponseWriter) WriteHeader(statusCode int) {
	t.statusCode = statusCode
}
