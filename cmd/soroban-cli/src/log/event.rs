use crate::xdr;
pub fn events(events: &[xdr::DiagnosticEvent]) {
    for (i, event) in events.iter().enumerate() {
        if is_contract_event(event) {
            tracing::info!(event_type = "contract", "{i}: {event:#?}");
        } else if is_log_event(event) {
            tracing::info!(event_type = "log", "{i}: {event:#?}");
        } else {
            tracing::debug!(event_type = "diagnostic", "{i}: {event:#?}");
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
