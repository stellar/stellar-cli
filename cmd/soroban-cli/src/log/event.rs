use soroban_spec_tools::Spec;
use tracing::debug;

use crate::{print::Print, xdr};
use xdr::{
    ContractEvent, ContractEventBody, ContractEventType, ContractEventV0, DiagnosticEvent, WriteXdr,
};

pub fn all(events: &[DiagnosticEvent]) {
    for (index, event) in events.iter().enumerate() {
        let json = serde_json::to_string(event).unwrap();
        let xdr = event.to_xdr_base64(xdr::Limits::none()).unwrap();
        print_event(&xdr, &json, index);
    }
}

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
            } => {
                if topics[0] == xdr::ScVal::Symbol(str_to_sc_symbol("log")) {
                    let status = if in_successful_contract_call {
                        "Success"
                    } else {
                        "Failure"
                    };

                    let data = serde_json::to_string(&data).unwrap();
                    print.logln(format!("{contract_id} - {status} - Log: {data}"));
                }
            }

            _ => {}
        }
    }
}

fn str_to_sc_symbol(s: &str) -> xdr::ScSymbol {
    let inner: xdr::StringM<32> = s.try_into().unwrap();
    xdr::ScSymbol(inner)
}

fn print_event(xdr: &str, json: &str, index: usize) {
    debug!("{index}: {xdr} {json}");
}
