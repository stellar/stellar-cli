use crate::xdr;
pub mod auth;
pub mod budget;
pub mod cost;
pub mod events;
pub mod footprint;
pub mod host_event;

pub use auth::*;
pub use budget::*;
pub use cost::*;
pub use events::*;
pub use footprint::*;
pub use host_event::*;

pub fn extract_events(tx_meta: &xdr::TransactionMeta) -> Vec<xdr::DiagnosticEvent> {
    match tx_meta {
        xdr::TransactionMeta::V3(xdr::TransactionMetaV3 {
            soroban_meta: Some(meta),
            ..
        }) => meta.diagnostic_events.to_vec(),
        _ => Vec::new(),
    }
}
