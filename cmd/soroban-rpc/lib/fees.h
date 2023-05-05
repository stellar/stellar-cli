// NOTE: You could use https://michael-f-bryan.github.io/rust-ffi-guide/cbindgen.html to generate
// this header automatically from your Rust code.  But for now, we'll just write it by hand.

#include <stdint.h>

typedef struct TransactionResources {
    /// Number of CPU instructions.
    uint32_t instructions;
    /// Number of ledger entries the transaction reads.
    uint32_t read_entries;
    /// Number of ledger entries the transaction writes (these are also counted
    /// as entries that are being read for the sake of the respective fees).
    uint32_t write_entries;
    /// Number of bytes read from ledger.
    uint32_t read_bytes;
    /// Number of bytes written to ledger.
    uint32_t write_bytes;
    /// Size of the metadata that transaction emits. Consists of the size of
    /// the events XDR, the size of writeable entries XDR before the transaction
    /// is applied, the size of writeable entries XDR after the transaction is
    /// applied.
    uint32_t metadata_size_bytes;
    /// Size of the transaction XDR.
    uint32_t transaction_size_bytes;

} TransactionResources;


/// Fee-related network configuration.
///
/// This should be normally loaded from the ledger.
typedef struct FeeConfiguration {
      /// Fee per `INSTRUCTIONS_INCREMENT=10000` instructions.
    int64_t fee_per_instruction_increment;
    /// Fee per 1 entry read from ledger.
    int64_t fee_per_read_entry;
    /// Fee per 1 entry written to ledger.
    int64_t fee_per_write_entry;
    /// Fee per 1KB read from ledger.
    int64_t fee_per_read_1kb;
    /// Fee per 1KB written to ledger.
    int64_t fee_per_write_1kb;
    /// Fee per 1KB written to history (the history write size is based on
    /// transaction size and `TX_BASE_RESULT_SIZE`).
    int64_t fee_per_historical_1kb;
    /// Fee per 1KB of metadata written.
    int64_t fee_per_metadata_1kb;
    /// Fee per 1KB propagate to the network (the propagated size is equal to
    /// the transaction size).
    int64_t pub fee_per_propagate_1kb;
} FeeConfiguration;

typedef struct ComputeTransactionResourceFeeResult {
    uint64_t fee;
    uint64_t refundable_fee;
} ComputeTransactionResourceFeeResult;

extern void ComputeTransactionResourceFee(
        const TransactionResources* transaction_resources,
        const FeeConfiguration* fee_configuration,
        ComputeTransactionResourceFeeResult* result,
    );

