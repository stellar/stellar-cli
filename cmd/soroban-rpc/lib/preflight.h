// NOTE: You could use https://michael-f-bryan.github.io/rust-ffi-guide/cbindgen.html to generate
// this header automatically from your Rust code.  But for now, we'll just write it by hand.

#include <stdint.h>
#include <stdbool.h>

typedef struct ledger_info_t {
  uint32_t protocol_version;
  uint32_t sequence_number;
  uint64_t timestamp;
  const char *network_passphrase;
  uint32_t base_reserve;
  uint32_t min_temp_entry_ttl;
  uint32_t min_persistent_entry_ttl;
  uint32_t max_entry_ttl;
} ledger_info_t;

typedef struct xdr_t {
    unsigned char *xdr;
    size_t        len;
} xdr_t;

typedef struct xdr_vector_t {
    xdr_t  *array;
    size_t len;
} xdr_vector_t;

typedef struct resource_config_t {
    uint64_t instruction_leeway; // Allow this many extra instructions when budgeting
} resource_config_t;

typedef struct preflight_result_t {
    char          *error; // Error string in case of error, otherwise null
    xdr_vector_t  auth; // array of SorobanAuthorizationEntries
    xdr_t         result; // XDR SCVal
    xdr_t         transaction_data;
    int64_t       min_fee; // Minimum recommended resource fee
    xdr_vector_t  events; // array of XDR DiagnosticEvents
    uint64_t      cpu_instructions;
    uint64_t      memory_bytes;
    xdr_t         pre_restore_transaction_data; // SorobanTransactionData XDR for a prerequired RestoreFootprint operation
    int64_t       pre_restore_min_fee; // Minimum recommended resource fee for a prerequired RestoreFootprint operation
} preflight_result_t;

preflight_result_t *preflight_invoke_hf_op(uintptr_t handle, // Go Handle to forward to SnapshotSourceGet
                                           uint64_t bucket_list_size, // Bucket list size of current ledger
                                           const xdr_t invoke_hf_op, // InvokeHostFunctionOp XDR
                                           const xdr_t source_account, // AccountId XDR
                                           const ledger_info_t ledger_info,
                                           const resource_config_t resource_config,
                                           bool enable_debug);

preflight_result_t *preflight_footprint_ttl_op(uintptr_t   handle, // Go Handle to forward to SnapshotSourceGet
                                               uint64_t bucket_list_size, // Bucket list size of current ledger
                                               const xdr_t op_body, // OperationBody XDR
                                               const xdr_t footprint, // LedgerFootprint XDR
                                               uint32_t    current_ledger_seq); // Current ledger sequence


// LedgerKey XDR to LedgerEntry XDR
extern xdr_t SnapshotSourceGet(uintptr_t handle, xdr_t ledger_key);

void free_preflight_result(preflight_result_t *result);

extern void FreeGoXDR(xdr_t xdr);
