use soroban_spec_tools::Spec;
use tracing::debug;

use crate::{print::Print, xdr};
use xdr::{ContractEvent, ContractEventBody, ContractEventType, ContractEventV0, DiagnosticEvent};

pub fn contract(events: &[DiagnosticEvent], print: &Print) {
    contract_with_spec(events, print, None);
}

/// Display contract events with self-describing format if spec is available
///
/// When a spec is provided, attempts to decode events using the spec to produce
/// human-readable output with named parameters. Falls back to raw format when:
/// - No spec is provided
/// - Event doesn't match any spec
/// - Decode fails
pub fn contract_with_spec(events: &[DiagnosticEvent], print: &Print, spec: Option<&Spec>) {
    for event in events.iter().cloned() {
        match event {
            DiagnosticEvent {
                event:
                    ContractEvent {
                        contract_id: Some(contract_id),
                        body: ContractEventBody::V0(ContractEventV0 { topics, data, .. }),
                        type_: ContractEventType::Contract,
                        ..
                    },
                in_successful_contract_call,
                ..
            } => {
                let status = if in_successful_contract_call {
                    "Success"
                } else {
                    "Failure"
                };

                // Try to decode with spec if available
                if let Some(spec) = spec {
                    let contract_id_str = contract_id.to_string();
                    match spec.decode_event(&contract_id_str, &topics, &data) {
                        Ok(decoded) => {
                            let params_str = decoded
                                .params
                                .iter()
                                .map(|(k, v)| format!("{k}: {v}"))
                                .collect::<Vec<_>>()
                                .join(", ");

                            let prefix_str = if decoded.prefix_topics.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", decoded.prefix_topics.join(", "))
                            };
                            let output = format!(
                                "{contract_id} - {status} - Event: {}{prefix_str}, {params_str}",
                                decoded.event_name
                            )
                            .trim_end_matches([',', ' '])
                            .to_string();

                            print.eventln(output);
                            continue;
                        }
                        Err(e) => {
                            // Event doesn't match the provided spec (likely from a different contract)
                            debug!(
                                "Event from {contract_id} not decoded: {e}. \
                                This may be a cross-contract event (e.g., token transfer) \
                                for which we don't have the spec."
                            );
                        }
                    }
                }

                // Fallback to raw format
                let topics_json = serde_json::to_string(&topics).unwrap();
                let data_json = serde_json::to_string(&data).unwrap();
                print.eventln(format!(
                    "{contract_id} - {status} - Event: {topics_json} = {data_json}"
                ));
            }

            DiagnosticEvent {
                event:
                    ContractEvent {
                        contract_id: Some(contract_id),
                        body: ContractEventBody::V0(ContractEventV0 { topics, data, .. }),
                        type_: ContractEventType::Diagnostic,
                        ..
                    },
                in_successful_contract_call,
                ..
            } if topics.first() == Some(&xdr::ScVal::Symbol(str_to_sc_symbol("log"))) => {
                let status = if in_successful_contract_call {
                    "Success"
                } else {
                    "Failure"
                };

                let data = serde_json::to_string(&data).unwrap();
                print.logln(format!("{contract_id} - {status} - Log: {data}"));
            }

            _ => {}
        }
    }
}

fn str_to_sc_symbol(s: &str) -> xdr::ScSymbol {
    let inner: xdr::StringM<32> = s.try_into().unwrap();
    xdr::ScSymbol(inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use xdr::{
        ContractEvent, ContractEventBody, ContractEventType, ContractEventV0, DiagnosticEvent,
        ScString, ScVal, VecM, WriteXdr,
    };

    struct BufWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for BufWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn make_diagnostic_event(payload: &str) -> DiagnosticEvent {
        DiagnosticEvent {
            in_successful_contract_call: true,
            event: ContractEvent {
                ext: xdr::ExtensionPoint::V0,
                contract_id: None,
                type_: ContractEventType::Diagnostic,
                body: ContractEventBody::V0(ContractEventV0 {
                    topics: VecM::default(),
                    data: ScVal::String(ScString(payload.try_into().unwrap())),
                }),
            },
        }
    }

    #[test]
    fn diagnostic_events_do_not_log_raw_payloads() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let buf_clone = buf.clone();

        let event = make_diagnostic_event("RAW_SECRET_PAYLOAD_12345");
        let event_xdr = event.to_xdr_base64(xdr::Limits::none()).unwrap();

        let subscriber = tracing_subscriber::fmt::Subscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || BufWriter(buf_clone.clone()))
            .finish();

        let print = Print::new(false);
        tracing::subscriber::with_default(subscriber, || {
            contract_with_spec(&[event], &print, None);
        });

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            !output.contains(&event_xdr),
            "event logging must not emit raw event XDR; got: {output}"
        );
        assert!(
            !output.contains("RAW_SECRET_PAYLOAD_12345"),
            "event logging must not emit raw event JSON payload; got: {output}"
        );
    }
}
