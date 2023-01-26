package methods

import (
	"context"
	"runtime/cgo"
	"unsafe"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/db"
)

//go:generate make -C ../../lib

/*
#include "../../lib/preflight.h"
#include <stdlib.h>
// This assumes that the Rust compiler should be using a -gnu target (i.e. MinGW compiler) in Windows
// (I (fons) am not even sure if CGo supports MSVC, see https://github.com/golang/go/issues/20982)
#cgo windows,amd64 LDFLAGS: -L${SRCDIR}/../../../../target/x86_64-pc-windows-gnu/release/ -lpreflight -ldl -lm -static -lws2_32 -lbcrypt -luserenv
// You cannot compile with -static in macOS (and it's not worth it in Linux, at least with glibc)
#cgo darwin,amd64 LDFLAGS: -L${SRCDIR}/../../../../target/x86_64-apple-darwin/release/ -lpreflight -ldl -lm
#cgo darwin,arm64 LDFLAGS: -L${SRCDIR}/../../../../target/aarch64-apple-darwin/release/ -lpreflight -ldl -lm
// In Linux, at least for now, we will be dynamically linking glibc. See https://github.com/2opremio/soroban-go-rust-preflight-poc/issues/3 for details
// I (fons) did try linking statically against musl but it caused problems catching (unwinding) Rust panics.
#cgo linux,amd64 LDFLAGS: -L${SRCDIR}/../../../../target/x86_64-unknown-linux-gnu/release/ -lpreflight -ldl -lm
#cgo linux,arm64 LDFLAGS: -L${SRCDIR}/../../../../target/aarch64-unknown-linux-gnu/release/ -lpreflight -ldl -lm
*/
import "C"

type snapshotSourceHandle struct {
	db     db.DB
	logger *log.Entry
}

// SnapshotSourceGet takes a LedgerKey XDR in base64 string and returns its matching LedgerEntry XDR in base64 string
// It's used by the Rust preflight code to obtain ledger entries.
//
//export SnapshotSourceGet
func SnapshotSourceGet(handle C.uintptr_t, ledger_key *C.char) *C.char {
	h := cgo.Handle(handle).Value().(snapshotSourceHandle)
	ledgerKeyB64 := C.GoString(ledger_key)
	var ledgerKey xdr.LedgerKey
	if err := xdr.SafeUnmarshalBase64(ledgerKeyB64, &ledgerKey); err != nil {
		h.logger.Errorf("SnapshotSourceGet(): failed to unmarshal ledger key passed from libpreflight: %v", err)
		return nil
	}
	entry, present, _, err := h.db.GetLedgerEntry(ledgerKey)
	if err != nil {
		h.logger.Errorf("SnapshotSourceGet(): GetLedgerEntry() failed: %v", err)
		return nil
	}
	if !present {
		return nil
	}
	out, err := xdr.MarshalBase64(entry)
	if err != nil {
		h.logger.Errorf("SnapshotSourceGet(): failed to marshal ledger entry from store: %v", err)
		return nil
	}
	return C.CString(out)
}

// SnapshotSourceHas takes LedgerKey XDR in base64 and returns whether it exists
// It's used by the Rust preflight code to obtain ledger entries.
//
//export SnapshotSourceHas
func SnapshotSourceHas(handle C.uintptr_t, ledger_key *C.char) C.int {
	h := cgo.Handle(handle).Value().(snapshotSourceHandle)
	ledgerKeyB64 := C.GoString(ledger_key)
	var ledgerKey xdr.LedgerKey
	if err := xdr.SafeUnmarshalBase64(ledgerKeyB64, &ledgerKey); err != nil {
		h.logger.Errorf("SnapshotSourceHas(): failed to unmarshal ledger key passed from libpreflight: %v", err)
		return 0
	}
	_, present, _, err := h.db.GetLedgerEntry(ledgerKey)
	if err != nil {
		h.logger.Errorf("SnapshotSourceHas(): GetLedgerEntry() failed: %v", err)
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

type SimulateTransactionRequest struct {
	Transaction string `json:"transaction"`
}

type SimulateTransactionCost struct {
	CPUInstructions uint64 `json:"cpuInsns,string"`
	MemoryBytes     uint64 `json:"memBytes,string"`
}

type InvokeHostFunctionResult struct {
	XDR string `json:"xdr"`
}

type SimulateTransactionResponse struct {
	Error        string                     `json:"error,omitempty"`
	Results      []InvokeHostFunctionResult `json:"results,omitempty"`
	Footprint    string                     `json:"footprint"`
	Cost         SimulateTransactionCost    `json:"cost"`
	LatestLedger int64                      `json:"latestLedger,string"`
}

// NewSimulateTransactionHandler returns a json rpc handler to run preflight simulations
func NewSimulateTransactionHandler(logger *log.Entry, networkPassphrase string, db db.DB) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request SimulateTransactionRequest) SimulateTransactionResponse {
		// TODO: this is racy, we need a read transaction for the whole request
		//       (otherwise we may end up supplying entries from different ledgers)
		latestLedger, err := db.GetLatestLedgerSequence()
		if err != nil {
			panic(err)
		}
		var txEnvelope xdr.TransactionEnvelope
		if err := xdr.SafeUnmarshalBase64(request.Transaction, &txEnvelope); err != nil {
			logger.WithError(err).WithField("request", request).
				Info("could not unmarshal simulate transaction envelope")
			return SimulateTransactionResponse{
				Error: "Could not unmarshal transaction",
			}
		}
		if len(txEnvelope.Operations()) != 1 {
			return SimulateTransactionResponse{
				Error: "Transaction contains more than one operation",
			}
		}
		op := txEnvelope.Operations()[0]

		var sourceAccount xdr.AccountId
		if opSourceAccount := op.SourceAccount; opSourceAccount != nil {
			sourceAccount = opSourceAccount.ToAccountId()
		} else {
			// FIXME: SourceAccount() panics, so, the user can doctor an envelope which makes the server crash
			sourceAccount = txEnvelope.SourceAccount().ToAccountId()
		}

		xdrOp, ok := op.Body.GetInvokeHostFunctionOp()
		if !ok {
			return SimulateTransactionResponse{
				Error: "Transaction does not contain invoke host function operation",
			}
		}

		li := C.CLedgerInfo{
			network_passphrase: C.CString(networkPassphrase),
			sequence_number:    C.uint(latestLedger),
			// TODO: find a way to fill these parameters appropriately
			protocol_version: 20,
			timestamp:        1,
			base_reserve:     1,
		}
		hfB64, err := xdr.MarshalBase64(xdrOp.Function)
		if err != nil {
			return SimulateTransactionResponse{
				Error: "Cannot marshal host function",
			}
		}
		hfCString := C.CString(hfB64)
		sourceAccountB64, err := xdr.MarshalBase64(sourceAccount)
		if err != nil {
			return SimulateTransactionResponse{
				Error: "Cannot marshal source account",
			}
		}

		handle := C.uintptr_t(cgo.NewHandle(snapshotSourceHandle{db, logger}))
		sourceAccountCString := C.CString(sourceAccountB64)
		res := C.preflight_host_function(
			handle,
			hfCString,
			sourceAccountCString,
			li,
		)
		C.free(unsafe.Pointer(hfCString))
		C.free(unsafe.Pointer(sourceAccountCString))
		defer C.free_preflight_result(res)

		if res.error != nil {
			return SimulateTransactionResponse{
				Error:        C.GoString(res.error),
				LatestLedger: int64(latestLedger),
			}
		}

		return SimulateTransactionResponse{
			Results:   []InvokeHostFunctionResult{{XDR: C.GoString(res.result)}},
			Footprint: C.GoString(res.preflight),
			Cost: SimulateTransactionCost{
				CPUInstructions: uint64(res.cpu_instructions),
				MemoryBytes:     uint64(res.memory_bytes),
			},
			LatestLedger: int64(latestLedger),
		}
	})
}
