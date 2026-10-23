use crate::xdr;

pub mod auth;
pub mod cost;
pub mod event;
pub mod footprint;
pub mod state_changes;

pub use auth::*;
pub use cost::*;
pub use footprint::*;
pub use state_changes::*;

pub fn extract_events(tx_meta: &xdr::TransactionMeta) -> Vec<xdr::DiagnosticEvent> {
    match tx_meta {
        xdr::TransactionMeta::V4(meta) => meta.diagnostic_events.to_vec(),

        xdr::TransactionMeta::V3(xdr::TransactionMetaV3 {
            soroban_meta: Some(meta),
            ..
        }) => meta.diagnostic_events.to_vec(),
        _ => Vec::new(),
    }
}
