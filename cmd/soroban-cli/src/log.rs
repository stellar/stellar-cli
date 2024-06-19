use crate::xdr;
pub mod auth;
pub mod budget;
pub mod contract_event;
pub mod cost;
pub mod diagnostic_event;
pub mod footprint;
pub mod host_event;

pub use auth::*;
pub use budget::*;
pub use contract_event::*;
pub use cost::*;
pub use diagnostic_event::*;
pub use footprint::*;
pub use host_event::*;

pub fn events(events: &[xdr::DiagnosticEvent]) {
    let (contract_events, other_events): (Vec<_>, Vec<_>) =
        events.iter().partition(|e| is_contract_event(e));
    contract_event::contract_events(&contract_events, tracing::Level::INFO);
    diagnostic_event::diagnostic_events(&other_events, tracing::Level::DEBUG);
}

pub fn extract_events(tx_meta: &xdr::TransactionMeta) -> Vec<xdr::DiagnosticEvent> {
    match tx_meta {
        xdr::TransactionMeta::V3(xdr::TransactionMetaV3 {
            soroban_meta: Some(meta),
            ..
        }) => {
            let mut events = meta.diagnostic_events.to_vec();
            // NOTE: we assume there can only be one operation, since we only send one
            if meta.events.len() >= 1 {
                events.extend(meta.events.iter().map(|e| xdr::DiagnosticEvent {
                    in_successful_contract_call: true,
                    event: e.clone(),
                }));
            };
            events
        }
        _ => Vec::new(),
    }
}

fn is_contract_event(event: &xdr::DiagnosticEvent) -> bool {
    matches!(event.event.type_, xdr::ContractEventType::Contract)
}
