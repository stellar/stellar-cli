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

var UnrecoverablePanicGroup = panicGroup{
	logPanicsToStdErr:  true,
	exitProcessOnPanic: true,
}

var RecoverablePanicGroup = panicGroup{
	logPanicsToStdErr:  true,
	exitProcessOnPanic: false,
}

type panicGroup struct {
	log                *log.Entry
	logPanicsToStdErr  bool
	exitProcessOnPanic bool
	panicsCounter      prometheus.Counter
}

func (pg *panicGroup) Log(log *log.Entry) *panicGroup {
	return &panicGroup{
		log:                log,
		logPanicsToStdErr:  pg.logPanicsToStdErr,
		exitProcessOnPanic: pg.exitProcessOnPanic,
		panicsCounter:      pg.panicsCounter,
	}
}

func (pg *panicGroup) Counter(counter prometheus.Counter) *panicGroup {
	return &panicGroup{
		log:                pg.log,
		logPanicsToStdErr:  pg.logPanicsToStdErr,
		exitProcessOnPanic: pg.exitProcessOnPanic,
		panicsCounter:      counter,
	}
}

// panicGroup give us the ability to spin a goroutine, with clear upfront definitions on what should be done in the
// case of an internal panic.
func (pg *panicGroup) Go(fn func()) {
	go func() {
		defer pg.recoverRoutine(fn)
		fn()
	}()
}

func (pg *panicGroup) recoverRoutine(fn func()) {
	recoverRes := recover()
	if recoverRes == nil {
		return
	}
	var cs []string
	if pg.log != nil {
		cs = getPanicCallStack(fn)
		for _, line := range cs {
			pg.log.Warn(line)
		}
	}
	if pg.logPanicsToStdErr {
		if len(cs) == 0 {
			cs = getPanicCallStack(fn)
		}
		for _, line := range cs {
			fmt.Fprintln(os.Stderr, line)
		}
	}

	if pg.panicsCounter != nil {
		pg.panicsCounter.Inc()
	}
	if pg.exitProcessOnPanic {
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
		// once we reached the panicGroup entry, stop.
		if strings.Contains(callStackLine, "(*panicGroup).Go") {
			break
		}
	}
	return outCallStack
}
