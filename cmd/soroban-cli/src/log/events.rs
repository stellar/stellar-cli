use crate::xdr;
pub fn log(event: &xdr::DiagnosticEvent) {
    tracing::info!("{event:#?}");
}

pub fn contract(event: &xdr::DiagnosticEvent) {
    tracing::info!("{event:#?}");
}

pub fn diagnostic(event: &xdr::DiagnosticEvent) {
    tracing::debug!("{event:#?}");
}

pub fn events(events: &[xdr::DiagnosticEvent]) {
    for event in events {
        if is_contract_event(event) {
            contract(event);
        } else if is_log_event(event) {
            log(event);
        } else {
            diagnostic(event);
        }
    }
}

fn is_contract_event(event: &xdr::DiagnosticEvent) -> bool {
    matches!(event.event.type_, xdr::ContractEventType::Contract)
}

fn is_log_event(event: &xdr::DiagnosticEvent) -> bool {
    match &event.event.body {
        xdr::ContractEventBody::V0(xdr::ContractEventV0 { topics, .. }) if topics.len() == 1 => {
            topics[0] == xdr::ScVal::Symbol(str_to_sc_symbol("log"))
        }
        xdr::ContractEventBody::V0(_) => false,
    }
}

fn str_to_sc_symbol(s: &str) -> xdr::ScSymbol {
    let inner: xdr::StringM<32> = s.try_into().unwrap();
    xdr::ScSymbol(inner)
}
