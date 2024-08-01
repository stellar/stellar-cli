use serde_json;
use soroban_sdk::xdr::WriteXdr;
use stellar_xdr::curr::{DiagnosticEvent, Limits, ReadXdr};

pub fn sim_diagnostic_events(events: &[String], level: tracing::Level) {
    tracing::debug!("test");
    let diagnostic_events: Vec<Result<DiagnosticEvent, _>> = events
        .iter()
        .map(|event_xdr| DiagnosticEvent::from_xdr_base64(event_xdr, Limits::none()))
        .collect();

    log_diagnostic_events(&diagnostic_events, events, level);
}

pub fn diagnostic_events(events: &[DiagnosticEvent], level: tracing::Level) {
    let diagnostic_events: Vec<Result<DiagnosticEvent, std::convert::Infallible>> =
        events.iter().map(|event| Ok(event.clone())).collect();

    let event_xdrs: Vec<String> = events
        .iter()
        .map(|event| event.to_xdr_base64(Limits::none()).unwrap_or_default())
        .collect();

    log_diagnostic_events(&diagnostic_events, &event_xdrs, level);
}

fn log_diagnostic_events<E: std::fmt::Debug>(
    diagnostic_events: &[Result<DiagnosticEvent, E>],
    event_xdrs: &[String],
    level: tracing::Level,
) {
    for (i, (event_result, event_xdr)) in diagnostic_events.iter().zip(event_xdrs).enumerate() {
        let json_result = event_result
            .as_ref()
            .ok()
            .and_then(|event| serde_json::to_string(event).ok());
        let log_message = match (event_result, json_result) {
            (Ok(_), Some(json)) => format!("{i}: {event_xdr:#?} {json}"),
            (Err(e), _) => {
                format!("{i}: {event_xdr:#?} Failed to decode DiagnosticEvent XDR: {e:#?}")
            }
            (Ok(_), None) => format!("{i}: {event_xdr:#?} JSON encoding of DiagnosticEvent failed"),
        };
        match level {
            tracing::Level::TRACE => tracing::trace!("{}", log_message),
            tracing::Level::INFO => tracing::info!("{}", log_message),
            tracing::Level::ERROR => tracing::error!("{}", log_message),
            _ => tracing::debug!("{}", log_message), // Default to debug for other levels
        }
    }
}
