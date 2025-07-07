use soroban_rpc::GetTransactionEvents;
use tracing::debug;

use crate::{print::Print, xdr};
use xdr::{
    ContractEvent, ContractEventBody, ContractEventType, ContractEventV0, DiagnosticEvent, WriteXdr,
};

pub fn all(events: &GetTransactionEvents) {
    let mut index = 0;

    for event in &events.diagnostic_events {
        index += 1;

        let json = serde_json::to_string(event).unwrap();
        let xdr = event.to_xdr_base64(xdr::Limits::none()).unwrap();
        print_event("diagnostic", &xdr, &json, index);
    }

    for event in &events.transaction_events {
        index += 1;

        let json = serde_json::to_string(event).unwrap();
        let xdr = event.to_xdr_base64(xdr::Limits::none()).unwrap();
        print_event("transaction", &xdr, &json, index);
    }

    for event in &events.contract_events {
        index += 1;

        let json = serde_json::to_string(event).unwrap();
        let xdr = event.to_xdr_base64(xdr::Limits::none()).unwrap();
        print_event("contract", &xdr, &json, index);
    }
}

pub fn contract(events: &GetTransactionEvents, print: &Print) {
    for event in &events.diagnostic_events {
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

    for event in &events.contract_events {
        match event {
            ContractEvent {
                contract_id: Some(contract_id),
                body: ContractEventBody::V0(ContractEventV0 { topics, data, .. }),
                ..
            } => {
                let topics = serde_json::to_string(&topics).unwrap();
                let data = serde_json::to_string(&data).unwrap();
                print.eventln(format!("{contract_id} - Event: {topics} = {data}"));
            }
            _ => panic!("Unhandled contract event: {event:?}"),
        }
    }
}

fn str_to_sc_symbol(s: &str) -> xdr::ScSymbol {
    let inner: xdr::StringM<32> = s.try_into().unwrap();
    xdr::ScSymbol(inner)
}

fn print_event(event_type: &str, xdr: &str, json: &str, index: u32) {
    debug!("{index}: [{event_type}] {xdr} {json}");
}
