package preflight

import (
	"context"
	"errors"
	"fmt"
	"runtime/cgo"
	"time"
	"unsafe"

	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

/*
#include "../../lib/preflight.h"
#include <stdlib.h>
// This assumes that the Rust compiler should be using a -gnu target (i.e. MinGW compiler) in Windows
// (I (fons) am not even sure if CGo supports MSVC, see https://github.com/golang/go/issues/20982)
#cgo windows,amd64 LDFLAGS: -L${SRCDIR}/../../../../target/x86_64-pc-windows-gnu/release-with-panic-unwind/ -lpreflight -lntdll -static -lws2_32 -lbcrypt -luserenv
// You cannot compile with -static in macOS (and it's not worth it in Linux, at least with glibc)
#cgo darwin,amd64 LDFLAGS: -L${SRCDIR}/../../../../target/x86_64-apple-darwin/release-with-panic-unwind/ -lpreflight -ldl -lm
#cgo darwin,arm64 LDFLAGS: -L${SRCDIR}/../../../../target/aarch64-apple-darwin/release-with-panic-unwind/ -lpreflight -ldl -lm
// In Linux, at least for now, we will be dynamically linking glibc. See https://github.com/2opremio/soroban-go-rust-preflight-poc/issues/3 for details
// I (fons) did try linking statically against musl but it caused problems catching (unwinding) Rust panics.
#cgo linux,amd64 LDFLAGS: -L${SRCDIR}/../../../../target/x86_64-unknown-linux-gnu/release-with-panic-unwind/ -lpreflight -ldl -lm
#cgo linux,arm64 LDFLAGS: -L${SRCDIR}/../../../../target/aarch64-unknown-linux-gnu/release-with-panic-unwind/ -lpreflight -ldl -lm
*/
import "C"

type snapshotSourceHandle struct {
	readTx db.LedgerEntryReadTx
	logger *log.Entry
}

// SnapshotSourceGet takes a LedgerKey XDR in base64 string and returns its matching LedgerEntry XDR in base64 string
// It's used by the Rust preflight code to obtain ledger entries.
//
//export SnapshotSourceGet
func SnapshotSourceGet(handle C.uintptr_t, cLedgerKey *C.char, includeExpired C.int) *C.char {
	h := cgo.Handle(handle).Value().(snapshotSourceHandle)
	ledgerKeyB64 := C.GoString(cLedgerKey)
	var ledgerKey xdr.LedgerKey
	if err := xdr.SafeUnmarshalBase64(ledgerKeyB64, &ledgerKey); err != nil {
		panic(err)
	}
	present, entry, err := h.readTx.GetLedgerEntry(ledgerKey, includeExpired != 0)
	if err != nil {
		h.logger.WithError(err).Error("SnapshotSourceGet(): GetLedgerEntry() failed")
		return nil
	}
	if !present {
		return nil
	}
	out, err := xdr.MarshalBase64(entry)
	if err != nil {
		panic(err)
	}
	return C.CString(out)
}

// SnapshotSourceHas takes LedgerKey XDR in base64 and returns whether it exists
// It's used by the Rust preflight code to obtain ledger entries.
//
//export SnapshotSourceHas
func SnapshotSourceHas(handle C.uintptr_t, cLedgerKey *C.char) C.int {
	h := cgo.Handle(handle).Value().(snapshotSourceHandle)
	ledgerKeyB64 := C.GoString(cLedgerKey)
	var ledgerKey xdr.LedgerKey
	if err := xdr.SafeUnmarshalBase64(ledgerKeyB64, &ledgerKey); err != nil {
		panic(err)
	}
	present, _, err := h.readTx.GetLedgerEntry(ledgerKey, false)
	if err != nil {
		h.logger.WithError(err).Error("SnapshotSourceHas(): GetLedgerEntry() failed")
		return 0
	}
	if present {
		return 1
	}
	return 0
}

//export FreeGoCString
func FreeGoCString(str *C.char) {
	C.free(unsafe.Pointer(str))
}

type PreflightParameters struct {
	Logger            *log.Entry
	SourceAccount     xdr.AccountId
	OpBody            xdr.OperationBody
	Footprint         xdr.LedgerFootprint
	NetworkPassphrase string
	LedgerEntryReadTx db.LedgerEntryReadTx
}

type Preflight struct {
	Events          []string // DiagnosticEvents XDR in base64
	TransactionData string   // SorobanTransactionData XDR in base64
	MinFee          int64
	Result          string   // XDR SCVal in base64
	Auth            []string // SorobanAuthorizationEntrys XDR in base64
	CPUInstructions uint64
	MemoryBytes     uint64
}

// GoNullTerminatedStringSlice transforms a C NULL-terminated char** array to a Go string slice
func GoNullTerminatedStringSlice(str **C.char) []string {
	var result []string
	if str != nil {
		// CGo doesn't have an easy way to do pointer arithmetic so,
		// we are better off transforming the memory buffer into a large slice
		// and finding the NULL termination after that
		for _, a := range unsafe.Slice(str, 1<<20) {
			if a == nil {
				// we found the ending nil
				break
			}
			result = append(result, C.GoString(a))
		}
	}
	return result
}

func GetPreflight(ctx context.Context, params PreflightParameters) (Preflight, error) {
	handle := cgo.NewHandle(snapshotSourceHandle{params.LedgerEntryReadTx, params.Logger})
	defer handle.Delete()
	switch params.OpBody.Type {
	case xdr.OperationTypeInvokeHostFunction:
		return getInvokeHostFunctionPreflight(params)
	case xdr.OperationTypeBumpFootprintExpiration, xdr.OperationTypeRestoreFootprint:
		return getFootprintExpirationPreflight(params)
	default:
		return Preflight{}, fmt.Errorf("unsupported operation type: %s", params.OpBody.Type.String())
	}
}

func getFootprintExpirationPreflight(params PreflightParameters) (Preflight, error) {
	opBodyB64, err := xdr.MarshalBase64(params.OpBody)
	if err != nil {
		return Preflight{}, err
	}
	opBodyCString := C.CString(opBodyB64)
	footprintB64, err := xdr.MarshalBase64(params.Footprint)
	if err != nil {
		return Preflight{}, err
	}
	footprintCString := C.CString(footprintB64)
	handle := cgo.NewHandle(snapshotSourceHandle{params.LedgerEntryReadTx, params.Logger})
	defer handle.Delete()

	res := C.preflight_footprint_expiration_op(
		C.uintptr_t(handle),
		opBodyCString,
		footprintCString,
	)

	C.free(unsafe.Pointer(opBodyCString))
	C.free(unsafe.Pointer(footprintCString))

	return GoPreflight(res)
}

func getInvokeHostFunctionPreflight(params PreflightParameters) (Preflight, error) {
	invokeHostFunctionB64, err := xdr.MarshalBase64(params.OpBody.MustInvokeHostFunctionOp())
	if err != nil {
		return Preflight{}, err
	}
	invokeHostFunctionCString := C.CString(invokeHostFunctionB64)
	sourceAccountB64, err := xdr.MarshalBase64(params.SourceAccount)
	if err != nil {
		return Preflight{}, err
	}
	latestLedger, err := params.LedgerEntryReadTx.GetLatestLedgerSequence()
	if err != nil {
		return Preflight{}, err
	}

	hasConfig, stateExpirationConfig, err := params.LedgerEntryReadTx.GetLedgerEntry(xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeConfigSetting,
		ConfigSetting: &xdr.LedgerKeyConfigSetting{
			ConfigSettingId: xdr.ConfigSettingIdConfigSettingStateExpiration,
		},
	}, false)
	if err != nil {
		return Preflight{}, err
	}
	minTempEntryExpiration := uint32(0)
	minPersistentEntryExpiration := uint32(0)
	maxEntryExpiration := uint32(0)
	if hasConfig {
		setting := stateExpirationConfig.Data.MustConfigSetting().MustStateExpirationSettings()
		minTempEntryExpiration = uint32(setting.MinTempEntryExpiration)
		minPersistentEntryExpiration = uint32(setting.MinPersistentEntryExpiration)
		maxEntryExpiration = uint32(setting.MaxEntryExpiration)
	}

	li := C.CLedgerInfo{
		network_passphrase: C.CString(params.NetworkPassphrase),
		sequence_number:    C.uint(latestLedger),
		protocol_version:   20,
		timestamp:          C.uint64_t(time.Now().Unix()),
		// Current base reserve is 0.5XLM (in stroops)
		base_reserve:                    5_000_000,
		min_temp_entry_expiration:       C.uint(minTempEntryExpiration),
		min_persistent_entry_expiration: C.uint(minPersistentEntryExpiration),
		max_entry_expiration:            C.uint(maxEntryExpiration),
	}

	sourceAccountCString := C.CString(sourceAccountB64)
	handle := cgo.NewHandle(snapshotSourceHandle{params.LedgerEntryReadTx, params.Logger})
	defer handle.Delete()
	res := C.preflight_invoke_hf_op(
		C.uintptr_t(handle),
		invokeHostFunctionCString,
		sourceAccountCString,
		li,
	)
	C.free(unsafe.Pointer(invokeHostFunctionCString))
	C.free(unsafe.Pointer(sourceAccountCString))

	return GoPreflight(res)
}

func GoPreflight(result *C.CPreflightResult) (Preflight, error) {
	defer C.free_preflight_result(result)

	if result.error != nil {
		return Preflight{}, errors.New(C.GoString(result.error))
	}

	preflight := Preflight{
		Events:          GoNullTerminatedStringSlice(result.events),
		TransactionData: C.GoString(result.transaction_data),
		MinFee:          int64(result.min_fee),
		Result:          C.GoString(result.result),
		Auth:            GoNullTerminatedStringSlice(result.auth),
		CPUInstructions: uint64(result.cpu_instructions),
		MemoryBytes:     uint64(result.memory_bytes),
	}
	return preflight, nil
}
