use soroban_env_host::xdr::DiagnosticEvent;

pub fn diagnostic_events(events: &[DiagnosticEvent], is_trace: bool) {
    for (i, event) in events.iter().enumerate() {
        if is_trace {
            tracing::trace!("{i}: {event:#?}");
        } else {
            tracing::info!("{i}: {event:#?}");
        }
    }
}
