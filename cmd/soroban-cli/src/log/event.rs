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

fn print_event(xdr: &str, json: &str, index: usize) {
    debug!("{index}: {xdr} {json}");
}
