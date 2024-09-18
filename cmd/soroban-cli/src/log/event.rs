use tracing::{debug, info, span, Level};

use crate::xdr;
use xdr::WriteXdr;

pub fn events(events: &[xdr::DiagnosticEvent]) {
    for (i, event) in events.iter().enumerate() {
        let span = if is_contract_event(event) {
            span!(Level::INFO, "contract_event")
        } else if is_log_event(event) {
            span!(Level::INFO, "log_event")
        } else {
            span!(Level::DEBUG, "diagnostic_event")
        };

        let _enter = span.enter();

        let xdr = event.to_xdr_base64(xdr::Limits::none()).unwrap();
        let json = serde_json::to_string(event).unwrap();
        if span.metadata().unwrap().level() == &Level::INFO {
            info!("{i}: {xdr} {json}");
        } else {
            debug!("{i}: {xdr} {json}");
        }
    }
}

fn is_contract_event(event: &xdr::DiagnosticEvent) -> bool {
    matches!(event.event.type_, xdr::ContractEventType::Contract)
}

fn is_log_event(event: &xdr::DiagnosticEvent) -> bool {
    match &event.event.body {
        xdr::ContractEventBody::V0(xdr::ContractEventV0 { topics, .. })
            if topics.len() == 1
                && matches!(event.event.type_, xdr::ContractEventType::Diagnostic) =>
        {
            topics[0] == xdr::ScVal::Symbol(str_to_sc_symbol("log"))
        }
        xdr::ContractEventBody::V0(_) => false,
    }
}

fn str_to_sc_symbol(s: &str) -> xdr::ScSymbol {
    let inner: xdr::StringM<32> = s.try_into().unwrap();
    xdr::ScSymbol(inner)
}
