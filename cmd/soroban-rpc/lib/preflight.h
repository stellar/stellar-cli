// NOTE: You could use https://michael-f-bryan.github.io/rust-ffi-guide/cbindgen.html to generate
// this header automatically from your Rust code.  But for now, we'll just write it by hand.

#include <stdint.h>

typedef struct CLedgerInfo {
  uint32_t protocol_version;
  uint32_t sequence_number;
  uint64_t timestamp;
  const char *network_passphrase;
  uint32_t base_reserve;
} CLedgerInfo;

typedef struct CPreflightResult {
    char *error; // Error string in case of error, otherwise null
    char *result; // SCVal XDR in base64
    char *preflight; // LedgerFootprint XDR in base64
    uint64_t cpu_instructions;
    uint64_t memory_bytes;
} CPreflightResult;

CPreflightResult *preflight_host_function(const char *hf, // HostFunction XDR in base64
                                          const char *args, // ScVec XDR in base64
                                          const char *source_account, // AccountId XDR in base64
                                          const struct CLedgerInfo ledger_info);

// LedgerKey XDR in base64 string to LedgerEntry XDR in base64 string
extern char *SnapshotSourceGet(char *ledger_key);

// LedgerKey XDR in base64 string to bool
extern int SnapshotSourceHas(char *ledger_key);

void free_preflight_result(CPreflightResult *result);

extern void FreeGoCString(char *str);

