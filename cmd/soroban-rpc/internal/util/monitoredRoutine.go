package util

import (
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
	if cfg.Log != nil {
		log := cfg.Log
		functionName := runtime.FuncForPC(reflect.ValueOf(fn).Pointer()).Name()
		log.Warnf("panicing root function '%s'", functionName)
		// while we're within the recoverRoutine, the debug.Stack() would return the
		// call stack where the panic took place.
		callStackStrings := string(debug.Stack())
		for i, callStackLine := range strings.FieldsFunc(callStackStrings, func(r rune) bool { return r == '\n' || r == '\t' }) {
			// skip the first 5 entries, since these are the "debug.Stack()" entries, which aren't really useful.
			if i < 5 {
				continue
			}
			log.Warn(callStackLine)
			// once we reached the MonitoredRoutine entry, stop.
			if strings.Contains(callStackLine, ".MonitoredRoutine") {
				break
			}
		}
	}
	if cfg.PanicsCounter != nil {
		cfg.PanicsCounter.Inc()
	}
	if cfg.ExitProcessOnPanic {
		os.Exit(1)
	}
}
