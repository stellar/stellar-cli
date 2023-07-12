// NOTE: You could use https://michael-f-bryan.github.io/rust-ffi-guide/cbindgen.html to generate
// this header automatically from your Rust code.  But for now, we'll just write it by hand.

#include <stdint.h>

typedef struct CLedgerInfo {
  uint32_t protocol_version;
  uint32_t sequence_number;
  uint64_t timestamp;
  const char *network_passphrase;
  uint32_t base_reserve;
  uint32_t min_temp_entry_expiration;
  uint32_t min_persistent_entry_expiration;
  uint32_t max_entry_expiration;
} CLedgerInfo;

typedef struct CPreflightResult {
    char *error; // Error string in case of error, otherwise null
    char **auth; // NULL terminated array of XDR SorobanAuthorizationEntrys in base64
    char *result; // XDR SCVal in base64
    char *transaction_data; // SorobanTransactionData XDR in base64
    int64_t min_fee; // Minimum recommended resource fee
    char **events; // NULL terminated array of XDR DiagnosticEvents in base64
    uint64_t cpu_instructions;
    uint64_t memory_bytes;
} CPreflightResult;

CPreflightResult *preflight_invoke_hf_op(uintptr_t handle, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHasconst
                                         const char *invoke_hf_op, // InvokeHostFunctionOp XDR in base64
                                         const char *source_account, // AccountId XDR in base64
                                         const struct CLedgerInfo ledger_info);

CPreflightResult *preflight_footprint_expiration_op(uintptr_t handle, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHasconst
                                                    const char *op_body, // OperationBody XDR in base64
                                                    const char *footprint); // LedgerFootprint XDR in base64

// LedgerKey XDR in base64 string to LedgerEntry XDR in base64 string
extern char *SnapshotSourceGet(uintptr_t handle, char *ledger_key, int include_expired);

// LedgerKey XDR in base64 string to bool
extern int SnapshotSourceHas(uintptr_t handle, char *ledger_key);

void free_preflight_result(CPreflightResult *result);

extern void FreeGoCString(char *str);

