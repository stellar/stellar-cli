use tracing::debug;

use crate::{print::Print, xdr};
use xdr::{
    ContractEvent, ContractEventBody, ContractEventType, ContractEventV0, DiagnosticEvent,
    TransactionMeta, WriteXdr,
};

pub fn all(events: &Vec<DiagnosticEvent>) {
    let mut index = 0;

    for event in events {
        index += 1;

        let json = serde_json::to_string(event).unwrap();
        let xdr = event.to_xdr_base64(xdr::Limits::none()).unwrap();
        print_event(&xdr, &json, index);
    }
}

pub fn get_diagnostic_events(meta: &TransactionMeta) -> Vec<DiagnosticEvent> {
    match meta {
        TransactionMeta::V4(meta) => meta.diagnostic_events.clone().into(),
        TransactionMeta::V3(xdr::TransactionMetaV3 {
            soroban_meta: Some(meta),
            ..
        }) => meta.diagnostic_events.clone().into(),
        _ => vec![],
    }
}

pub fn contract(events: &[DiagnosticEvent], print: &Print) {
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
                ..
            } => {
                let topics = serde_json::to_string(&topics).unwrap();
                let data = serde_json::to_string(&data).unwrap();
                print.eventln(format!("{contract_id} - Event: {topics} = {data}"));
            }

            DiagnosticEvent {
                event:
                    ContractEvent {
                        contract_id: Some(contract_id),
                        body: ContractEventBody::V0(ContractEventV0 { topics, data, .. }),
                        type_: ContractEventType::Diagnostic,
                        ..
                    },
                ..
            } => {
                if topics[0] == xdr::ScVal::Symbol(str_to_sc_symbol("log")) {
                    let data = serde_json::to_string(&data).unwrap();
                    print.logln(format!("{contract_id} - Log: {data}"));
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

fn print_event(xdr: &str, json: &str, index: u32) {
    debug!("{index}: {xdr} {json}");
}
