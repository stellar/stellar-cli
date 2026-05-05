use soroban_spec_tools::event::DecodedEvent;
use soroban_spec_tools::{sanitize, Spec};
use tracing::debug;

use crate::{print::Print, xdr};
use xdr::{
    ContractEvent, ContractEventBody, ContractEventType, ContractEventV0, DiagnosticEvent, WriteXdr,
};

fn format_decoded_event(decoded: &DecodedEvent, status: &str) -> String {
    let params_str = decoded
        .params
        .iter()
        .map(|(k, v)| format!("{}: {v}", sanitize(k)))
        .collect::<Vec<_>>()
        .join(", ");

    let prefix_str = if decoded.prefix_topics.is_empty() {
        String::new()
    } else {
        let prefix = decoded
            .prefix_topics
            .iter()
            .map(|t| sanitize(t))
            .collect::<Vec<_>>()
            .join(", ");
        format!(" ({prefix})")
    };

    format!(
        "{} - {status} - Event: {}{prefix_str}, {params_str}",
        decoded.contract_id,
        sanitize(&decoded.event_name),
    )
    .trim_end_matches([',', ' '])
    .to_string()
}

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
                            print.eventln(format_decoded_event(&decoded, status));
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

fn print_event(xdr: &str, json: &str, index: usize) {
    debug!("{index}: {xdr} {json}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use serde_json::json;
    use soroban_spec_tools::test_utils::assert_no_control_chars;

    #[test]
    fn format_decoded_event_strips_attacker_control_bytes() {
        let mut params = IndexMap::new();
        params.insert("amount\x1b[31m".to_string(), json!(1000));
        let decoded = DecodedEvent {
            contract_id: "CACA".to_string(),
            event_name: "\x1b[2J\x1b[Htransfer".to_string(),
            prefix_topics: vec!["\x1b[31mEVIL".into(), "topic2".into()],
            params,
        };

        assert_no_control_chars(&format_decoded_event(&decoded, "Success"));
    }
}
