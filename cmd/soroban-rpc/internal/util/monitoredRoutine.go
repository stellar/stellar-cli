package util

import (
	"fmt"
	"os"
	"reflect"
	"runtime"
	"runtime/debug"
	"strings"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/stellar/go/support/log"
)

type MonitoredRoutineConfiguration struct {
	Log                *log.Entry
	LogPanicsToStdErr  bool
	ExitProcessOnPanic bool
	PanicsCounter      prometheus.Counter
}

// MonitoredRoutine give us the ability to spin a goroutine, with clear upfront definitions on what should be done in the
// case of an internal panic.
func MonitoredRoutine(cfg MonitoredRoutineConfiguration, fn func()) {
	go func() {
		defer recoverRoutine(&cfg, fn)
		fn()
	}()
}

func recoverRoutine(cfg *MonitoredRoutineConfiguration, fn func()) {
	recoverRes := recover()
	if recoverRes == nil {
		return
	}
	var cs []string
	if cfg.Log != nil {
		cs = getPanicCallStack(fn)
		for _, line := range cs {
			cfg.Log.Warn(line)
		}
	}
	if cfg.LogPanicsToStdErr {
		if len(cs) == 0 {
			cs = getPanicCallStack(fn)
		}
		for _, line := range cs {
			fmt.Fprintln(os.Stderr, line)
		}
	}

	if cfg.PanicsCounter != nil {
		cfg.PanicsCounter.Inc()
	}
	if cfg.ExitProcessOnPanic {
		os.Exit(1)
	}
}

func getPanicCallStack(fn func()) (outCallStack []string) {
	functionName := runtime.FuncForPC(reflect.ValueOf(fn).Pointer()).Name()
	outCallStack = append(outCallStack, fmt.Sprintf("panicing root function '%s'", functionName))
	// while we're within the recoverRoutine, the debug.Stack() would return the
	// call stack where the panic took place.
	callStackStrings := string(debug.Stack())
	for i, callStackLine := range strings.FieldsFunc(callStackStrings, func(r rune) bool { return r == '\n' || r == '\t' }) {
		// skip the first 5 entries, since these are the "debug.Stack()" entries, which aren't really useful.
		if i < 5 {
			continue
		}
		outCallStack = append(outCallStack, callStackLine)
		// once we reached the MonitoredRoutine entry, stop.
		if strings.Contains(callStackLine, ".MonitoredRoutine") {
			break
		}
	}
	return outCallStack
}
