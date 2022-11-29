package methods

import (
	"context"
	"fmt"
	"unsafe"

	"github.com/stellar/go/support/log"
	"github.com/stellar/go/xdr"

	"github.com/creachadair/jrpc2"
	"github.com/creachadair/jrpc2/handler"
)

//go:generate make -C ../../lib

/*
#include "../../lib/preflight.h"
#include <stdlib.h>
// This assumes that the Rust compiler should be using a -gnu target (i.e. MinGW compiler) in Windows
// (I (fons) am not even sure if CGo supports MSVC, see https://github.com/golang/go/issues/20982)
#cgo windows,amd64 LDFLAGS: -L${SRCDIR}/../../lib/preflight/target/x86_64-pc-windows-gnu/release/ -lpreflight -ldl -lm -static -lws2_32 -lbcrypt -luserenv
// You cannot compile with -static in macOS (and it's not worth it in Linux, at least with glibc)
#cgo darwin,amd64 LDFLAGS: -L${SRCDIR}/../../lib/preflight/target/x86_64-apple-darwin/release/ -lpreflight -ldl -lm
#cgo darwin,arm64 LDFLAGS: -L${SRCDIR}/../../lib/preflight/target/aarch64-apple-darwin/release/ -lpreflight -ldl -lm
// In Linux, at least for now, we will be dynamically linking glibc. See https://github.com/2opremio/soroban-go-rust-preflight-poc/issues/3 for details
// I (fons) did try linking statically against musl but it caused problems catching (unwinding) Rust panics.
#cgo linux,amd64 LDFLAGS: -L${SRCDIR}/../../lib/preflight/target/x86_64-unknown-linux-gnu/release/ -lpreflight -ldl -lm
#cgo linux,arm64 LDFLAGS: -L${SRCDIR}/../../lib/preflight/target/aarch64-unknown-linux-gnu/release/ -lpreflight -ldl -lm
*/
import "C"

// SnapshotSourceGet takes a LedgerKey XDR in base64 string and returns its matching LedgerEntry XDR in base64 string
// It's used by the Rust preflight code to obtain ledger entries.
//
//export SnapshotSourceGet
func SnapshotSourceGet(ledger_key *C.char) *C.char {
	// TODO: we need a way to obtain raw ledger entries
	fmt.Println("gets to SnapshotSourceGet()")
	return nil
}

// SnapshotSourceHas takes LedgerKey XDR in base64 and returns whether it exists
// It's used by the Rust preflight code to obtain ledger entries.
//
//export SnapshotSourceHas
func SnapshotSourceHas(ledger_key *C.char) C.int {
	// TODO: we need a way to obtain raw ledger entries
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
func NewSimulateTransactionHandler(logger *log.Entry, networkPassphrase string) jrpc2.Handler {
	return handler.New(func(ctx context.Context, request SimulateTransactionRequest) SimulateTransactionResponse {
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
			// TODO: find a way to fill these parameters appropriately
			protocol_version: 20,
			sequence_number:  4000,
			timestamp:        1,
			base_reserve:     1,
		}
		hfB64, err := xdr.MarshalBase64(xdrOp.Function)
		if err != nil {
			return SimulateTransactionResponse{
				Error: "Cannot marshal host function",
			}
		}
		argsB64, err := xdr.MarshalBase64(xdrOp.Parameters)
		if err != nil {
			return SimulateTransactionResponse{
				Error: "Cannot marshal host function parameters",
			}
		}
		sourceAccountB64, err := xdr.MarshalBase64(sourceAccount)
		if err != nil {
			return SimulateTransactionResponse{
				Error: "Cannot marshal source account",
			}
		}
		argsCString := C.CString(argsB64)
		sourceAccountCString := C.CString(sourceAccountB64)
		res := C.preflight_host_function(C.CString(hfB64),
			argsCString,
			sourceAccountCString,
			li,
		)
		C.free(unsafe.Pointer(argsCString))
		C.free(unsafe.Pointer(sourceAccountCString))
		defer C.free_preflight_result(res)

		if res.error != nil {
			return SimulateTransactionResponse{
				Error: C.GoString(res.error),
				// TODO: how to fill the latest ledger?
				LatestLedger: 4000,
			}
		}

		return SimulateTransactionResponse{
			Results:   []InvokeHostFunctionResult{{XDR: C.GoString(res.result)}},
			Footprint: C.GoString(res.preflight),
			Cost: SimulateTransactionCost{
				CPUInstructions: uint64(res.cpu_instructions),
				MemoryBytes:     uint64(res.memory_bytes),
			},
			// TODO: how to fill the latest ledger?
			LatestLedger: 4000,
		}
	})
}
