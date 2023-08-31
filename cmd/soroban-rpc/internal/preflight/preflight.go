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
func SnapshotSourceGet(handle C.uintptr_t, cLedgerKey C.xdr_t) C.xdr_t {
	h := cgo.Handle(handle).Value().(snapshotSourceHandle)
	ledgerKeyXDR := GoXDR(cLedgerKey)
	var ledgerKey xdr.LedgerKey
	if err := xdr.SafeUnmarshal(ledgerKeyXDR, &ledgerKey); err != nil {
		panic(err)
	}
	present, entry, err := db.GetLedgerEntry(h.readTx, ledgerKey)
	if err != nil {
		h.logger.WithError(err).Error("SnapshotSourceGet(): GetLedgerEntry() failed")
		return C.xdr_t{}
	}
	if !present {
		return C.xdr_t{}
	}
	out, err := entry.MarshalBinary()
	if err != nil {
		panic(err)
	}

	return C.xdr_t{
		xdr: (*C.uchar)(C.CBytes(out)),
		len: C.size_t(len(out)),
	}
}

//export FreeGoXDR
func FreeGoXDR(xdr C.xdr_t) {
	C.free(unsafe.Pointer(xdr.xdr))
}

type PreflightParameters struct {
	Logger            *log.Entry
	SourceAccount     xdr.AccountId
	OpBody            xdr.OperationBody
	Footprint         xdr.LedgerFootprint
	NetworkPassphrase string
	LedgerEntryReadTx db.LedgerEntryReadTx
	BucketListSize    uint64
}

type Preflight struct {
	Error                     string
	Events                    [][]byte // DiagnosticEvents XDR
	TransactionData           []byte   // SorobanTransactionData XDR
	MinFee                    int64
	Result                    []byte   // XDR SCVal in base64
	Auth                      [][]byte // SorobanAuthorizationEntries XDR
	CPUInstructions           uint64
	MemoryBytes               uint64
	PreRestoreTransactionData []byte // SorobanTransactionData XDR
	PreRestoreMinFee          int64
}

func CXDR(xdr []byte) C.xdr_t {
	return C.xdr_t{
		xdr: (*C.uchar)(C.CBytes(xdr)),
		len: C.size_t(len(xdr)),
	}
}

func GoXDR(xdr C.xdr_t) []byte {
	return C.GoBytes(unsafe.Pointer(xdr.xdr), C.int(xdr.len))
}

func GoXDRVector(xdrVector C.xdr_vector_t) [][]byte {
	result := make([][]byte, xdrVector.len)
	inputSlice := unsafe.Slice(xdrVector.array, xdrVector.len)
	for i, v := range inputSlice {
		result[i] = GoXDR(v)
	}
	return result
}

func GetPreflight(ctx context.Context, params PreflightParameters) (Preflight, error) {
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
	opBodyXDR, err := params.OpBody.MarshalBinary()
	if err != nil {
		return Preflight{}, err
	}
	opBodyCXDR := CXDR(opBodyXDR)
	footprintXDR, err := params.Footprint.MarshalBinary()
	if err != nil {
		return Preflight{}, err
	}
	footprintCXDR := CXDR(footprintXDR)
	handle := cgo.NewHandle(snapshotSourceHandle{params.LedgerEntryReadTx, params.Logger})
	defer handle.Delete()

	simulationLedgerSeq, err := getSimulationLedgerSeq(params.LedgerEntryReadTx)
	if err != nil {
		return Preflight{}, err
	}

	res := C.preflight_footprint_expiration_op(
		C.uintptr_t(handle),
		C.uint64_t(params.BucketListSize),
		opBodyCXDR,
		footprintCXDR,
		C.uint32_t(simulationLedgerSeq),
	)

	FreeGoXDR(opBodyCXDR)
	FreeGoXDR(footprintCXDR)

	return GoPreflight(res), nil
}

func getSimulationLedgerSeq(readTx db.LedgerEntryReadTx) (uint32, error) {
	latestLedger, err := readTx.GetLatestLedgerSequence()
	if err != nil {
		return 0, err
	}
	// It's of utmost importance to simulate the transactions like we were on the next ledger.
	// Otherwise, users would need to wait for an extra ledger to close in order to observe the effects of the latest ledger
	// transaction submission.
	sequenceNumber := latestLedger + 1
	return sequenceNumber, nil
}

func getInvokeHostFunctionPreflight(params PreflightParameters) (Preflight, error) {
	invokeHostFunctionXDR, err := params.OpBody.MustInvokeHostFunctionOp().MarshalBinary()
	if err != nil {
		return Preflight{}, err
	}
	invokeHostFunctionCXDR := CXDR(invokeHostFunctionXDR)
	sourceAccountXDR, err := params.SourceAccount.MarshalBinary()
	if err != nil {
		return Preflight{}, err
	}
	sourceAccountCXDR := CXDR(sourceAccountXDR)

	hasConfig, stateExpirationConfig, err := db.GetLedgerEntry(params.LedgerEntryReadTx, xdr.LedgerKey{
		Type: xdr.LedgerEntryTypeConfigSetting,
		ConfigSetting: &xdr.LedgerKeyConfigSetting{
			ConfigSettingId: xdr.ConfigSettingIdConfigSettingStateExpiration,
		},
	})
	if err != nil {
		return Preflight{}, err
	}
	if !hasConfig {
		return Preflight{}, errors.New("state expiration config setting missing in ledger storage")
	}

	simulationLedgerSeq, err := getSimulationLedgerSeq(params.LedgerEntryReadTx)
	if err != nil {
		return Preflight{}, err
	}

	stateExpiration := stateExpirationConfig.Data.MustConfigSetting().MustStateExpirationSettings()
	li := C.ledger_info_t{
		network_passphrase: C.CString(params.NetworkPassphrase),
		sequence_number:    C.uint32_t(simulationLedgerSeq),
		protocol_version:   20,
		timestamp:          C.uint64_t(time.Now().Unix()),
		// Current base reserve is 0.5XLM (in stroops)
		base_reserve:                    5_000_000,
		min_temp_entry_expiration:       C.uint(stateExpiration.MinTempEntryExpiration),
		min_persistent_entry_expiration: C.uint(stateExpiration.MinPersistentEntryExpiration),
		max_entry_expiration:            C.uint(stateExpiration.MaxEntryExpiration),
	}

	handle := cgo.NewHandle(snapshotSourceHandle{params.LedgerEntryReadTx, params.Logger})
	defer handle.Delete()
	res := C.preflight_invoke_hf_op(
		C.uintptr_t(handle),
		C.uint64_t(params.BucketListSize),
		invokeHostFunctionCXDR,
		sourceAccountCXDR,
		li,
	)
	FreeGoXDR(invokeHostFunctionCXDR)
	FreeGoXDR(sourceAccountCXDR)

	return GoPreflight(res), nil
}

func GoPreflight(result *C.preflight_result_t) Preflight {
	defer C.free_preflight_result(result)

	preflight := Preflight{
		Error:                     C.GoString(result.error),
		Events:                    GoXDRVector(result.events),
		TransactionData:           GoXDR(result.transaction_data),
		MinFee:                    int64(result.min_fee),
		Result:                    GoXDR(result.result),
		Auth:                      GoXDRVector(result.auth),
		CPUInstructions:           uint64(result.cpu_instructions),
		MemoryBytes:               uint64(result.memory_bytes),
		PreRestoreTransactionData: GoXDR(result.pre_restore_transaction_data),
		PreRestoreMinFee:          int64(result.pre_restore_min_fee),
	}
	return preflight
}
