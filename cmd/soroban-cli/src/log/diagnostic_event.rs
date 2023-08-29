use soroban_env_host::xdr::DiagnosticEvent;

pub fn diagnostic_events(events: &[DiagnosticEvent]) {
    for (i, event) in events.iter().enumerate() {
        tracing::info!("{i}: {event:#?}");
    }
}
