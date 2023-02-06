// NOTE: You could use https://michael-f-bryan.github.io/rust-ffi-guide/cbindgen.html to generate
// this header automatically from your Rust code.  But for now, we'll just write it by hand.

#include <stdint.h>

typedef struct CLedgerInfo {
  uint32_t protocol_version;
  uint32_t sequence_number;
  uint64_t timestamp;
  uint8_t *network_id;
  uint32_t base_reserve;
} CLedgerInfo;

typedef struct CRecordedAuthPayload {
    const char *address;
    uint64_t *nonce;
    const char *invocation;
} CRecordedAuthPayload;

typedef struct CPreflightResult {
    char *error; // Error string in case of error, otherwise null
    char *result; // SCVal XDR in base64
    char *footprint; // LedgerFootprint XDR in base64
    char *auth; // Array<ContractAuth> XDR in base64
    const CRecordedAuthPayload *auth_ptr; // Array<CRecordedAuthPayload>
    uintptr_t auth_len;
    uintptr_t auth_cap;
    uint64_t cpu_instructions;
    uint64_t memory_bytes;
} CPreflightResult;

CPreflightResult *preflight_host_function(uintptr_t handle, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHasconst
                                          char *hf, // HostFunction XDR in base64
                                          const char *source_account, // AccountId XDR in base64
                                          const struct CLedgerInfo ledger_info);

// LedgerKey XDR in base64 string to LedgerEntry XDR in base64 string
extern char *SnapshotSourceGet(uintptr_t handle, char *ledger_key);

// LedgerKey XDR in base64 string to bool
extern int SnapshotSourceHas(uintptr_t handle, char *ledger_key);

void free_preflight_result(CPreflightResult *result);

extern void FreeGoCString(char *str);

