package util

import (
	"os"
	"sync"
	"testing"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/stellar/go/support/log"
	"github.com/stretchr/testify/require"
)

func TestMonitoredRoutineTrivial(t *testing.T) {
	ch := make(chan int)

	MonitoredRoutine(MonitoredRoutineConfiguration{}, func() { ch <- 1 })

	<-ch
}

type TestLogsCounter struct {
	entry             *log.Entry
	mu                sync.Mutex
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
	te.mu.Lock()
	defer te.mu.Unlock()
	te.writtenLogEntries[e.Level]++
	return nil
}
func (te *TestLogsCounter) GetLevel(i int) int {
	te.mu.Lock()
	defer te.mu.Unlock()
	return te.writtenLogEntries[i]
}

func PanicingFunctionA(w *int) {
	*w = 0
}

func IndirectPanicingFunctionB() {
	PanicingFunctionA(nil)
}

func IndirectPanicingFunctionC() {
	IndirectPanicingFunctionB()
}

func TestMonitoredRoutineLog(t *testing.T) {
	logCounter := makeTestLogCounter()
	MonitoredRoutine(MonitoredRoutineConfiguration{
		Log: logCounter.Entry(),
	}, IndirectPanicingFunctionC)
	// wait until we get all the log entries.
	waitStarted := time.Now()
	for time.Since(waitStarted) < 5*time.Second {
		warningCount := logCounter.GetLevel(3)
		if warningCount >= 10 {
			return
		}
		time.Sleep(1 * time.Millisecond)
	}
	t.FailNow()
}

func TestMonitoredRoutineStdErr(t *testing.T) {
	tmpFile, err := os.CreateTemp("", "TestMonitoredRoutine")
	require.NoError(t, err)
	defaultStdErr := os.Stderr
	os.Stderr = tmpFile
	defer func() {
		os.Stderr = defaultStdErr
		tmpFile.Close()
		os.Remove(tmpFile.Name())
	}()
	MonitoredRoutine(MonitoredRoutineConfiguration{
		LogPanicsToStdErr: true,
	}, IndirectPanicingFunctionC)
	// wait until we get all the log entries.
	waitStarted := time.Now()
	for time.Since(waitStarted) < 5*time.Second {
		outErrBytes, err := os.ReadFile(tmpFile.Name())
		require.NoError(t, err)
		if len(outErrBytes) >= 100 {
			return
		}
		time.Sleep(1 * time.Millisecond)
	}
	t.FailNow()
}
