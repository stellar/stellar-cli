use std::{fmt::Debug, io, io::Cursor, net::SocketAddr, path::PathBuf, rc::Rc, sync::Arc};

use clap::Parser;
use hex::FromHexError;
use serde_json::{json, Value};
use soroban_env_host::{
    storage::Storage,
    xdr::{
        self, Error as XdrError, HostFunction, ReadXdr, ScHostStorageErrorCode, ScObject, ScStatus,
        ScVal, WriteXdr,
        OperationBody, TransactionEnvelope,
        FeeBumpTransactionInnerTx,
        LedgerKey,
    },
    Host, HostError, Vm,
};
use warp::Filter;

use crate::contractspec;
use crate::jsonrpc;
use crate::snapshot;
use crate::strval::{self, StrValError};
use crate::utils;

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Port to listen for requests on.
    #[clap(long, default_value("8080"))]
    port: u16,
    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("strval")]
    StrVal(#[from] StrValError),
    #[error("xdr")]
    Xdr(#[from] XdrError),
    #[error("host")]
    Host(#[from] HostError),
    #[error("snapshot")]
    Snapshot(#[from] snapshot::Error),
    #[error("serde")]
    Serde(#[from] serde_json::Error),
    #[error("hex")]
    FromHex(#[from] FromHexError),
    #[error("contractnotfound")]
    FunctionNotFoundInContractSpec,
    #[error("unknownmethod")]
    UnknownMethod,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
enum Requests {
    SimulateTransaction(Box<[String]>)
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let ledger_file = Arc::new(self.ledger_file.clone());
        let with_ledger_file = warp::any().map(move || ledger_file.clone());

        let routes = warp::post()
            .and(warp::path!("api" / "v1" / "jsonrpc"))
            .and(warp::body::json())
            .and(with_ledger_file)
            .map(
                |request: jsonrpc::Request<Requests>, ledger_file: Arc<PathBuf>| {
                    if request.jsonrpc != "2.0" {
                        return json!({
                            "jsonrpc": "2.0",
                            "id": &request.id,
                            "error": {
                                "code":-32600,
                                "message": "Invalid jsonrpc value in request",
                            },
                        })
                        .to_string();
                    }
                    let result = match (request.method.as_str(), request.params) {
                        (
                            "simulateTransaction",
                            Some(Requests::SimulateTransaction(b)),
                        ) => {
                            if let Some(txn_xdr) = b.into_vec().first() {
                                simulate_transaction(&txn_xdr, &ledger_file)
                            } else {
                                Err(Error::Xdr(XdrError::Invalid))
                            }
                        }
                        _ => Err(Error::UnknownMethod),
                    };
                    let r = reply(&request.id, result);
                    serde_json::to_string(&r).unwrap_or_else(|_| {
                        json!({
                            "jsonrpc": "2.0",
                            "id": &request.id,
                            "error": {
                                "code":-32603,
                                "message": "Internal server error",
                            },
                        })
                        .to_string()
                    })
                },
            );

        let addr: SocketAddr = ([127, 0, 0, 1], self.port).into();
        println!("Listening on: {}", addr);
        warp::serve(routes).run(addr).await;
        Ok(())
    }
}

fn reply(
    id: &Option<jsonrpc::Id>,
    result: Result<Value, Error>,
) -> jsonrpc::Response<Value, Value> {
    match result {
        Ok(res) => {
            jsonrpc::Response::Ok(jsonrpc::ResultResponse {
                jsonrpc: "2.0".to_string(),
                id: id.as_ref().unwrap_or(&jsonrpc::Id::Null).clone(),
                result: res,
            })
        }
        Err(err) => {
            eprintln!("err: {:?}", err);
            jsonrpc::Response::Err(jsonrpc::ErrorResponse {
                jsonrpc: "2.0".to_string(),
                id: id.as_ref().unwrap_or(&jsonrpc::Id::Null).clone(),
                error: jsonrpc::ErrorResponseError {
                    code: match err {
                        Error::Serde(_) => -32700,
                        Error::UnknownMethod => -32601,
                        _ => -32603,
                    },
                    message: err.to_string(),
                    data: None,
                },
            })
        },
    }
}

fn simulate_transaction(
    txn_xdr: &str,
    ledger_file: &PathBuf,
) -> Result<Value, Error> {
    // Parse and validate the txn
    let decoded = base64::decode(txn_xdr).map_err(|_| Error::Xdr(XdrError::Invalid))?;
    let ops = match TransactionEnvelope::from_xdr(decoded)? {
        TransactionEnvelope::TxV0(envelope) => envelope.tx.operations,
        TransactionEnvelope::Tx(envelope) => envelope.tx.operations,
        TransactionEnvelope::TxFeeBump(envelope) => {
            let FeeBumpTransactionInnerTx::Tx(tx_envelope) = envelope.tx.inner_tx;
            tx_envelope.tx.operations
        },
    };
    if ops.len() != 1 {
        return Err(Error::Xdr(XdrError::Invalid));
    }
    let op = ops.first().ok_or(Error::Xdr(XdrError::Invalid))?;
    let body = if let OperationBody::InvokeHostFunction(b) = &op.body {
        b
    } else {
        return Err(Error::Xdr(XdrError::Invalid));
    };

    // pub function: HostFunction,
    // pub parameters: ScVec,
    // pub footprint: LedgerFootprint,
    if body.function != HostFunction::Call {
        return Err(Error::Xdr(XdrError::Invalid));
    };
    // contract: &String,
    // func: &String,
    // args: &[Value],
    // args_xdr: &[String],

    if body.parameters.len() < 2 {
        return Err(Error::Xdr(XdrError::Invalid));
    };

    let contract_xdr = body.parameters.get(0).ok_or(Error::Xdr(XdrError::Invalid))?;
    let method_xdr = body.parameters.get(1).ok_or(Error::Xdr(XdrError::Invalid))?;
    let (_, params) = body.parameters.split_at(2);

    let contract_id: [u8; 32] = if let ScVal::Object(Some(ScObject::Bytes(bytes))) = contract_xdr {
        bytes.as_slice().try_into().or_else(|_| Err(Error::Xdr(XdrError::Invalid)))?
    } else {
        return Err(Error::Xdr(XdrError::Invalid));
    };

    let method: String = if let ScVal::Object(Some(ScObject::Bytes(bytes))) = method_xdr {
        bytes.try_into().or_else(|_| Err(Error::Xdr(XdrError::Invalid)))?
    } else {
        return Err(Error::Xdr(XdrError::Invalid));
    };


    // Initialize storage and host
    // TODO: allow option to separate input and output file
    let ledger_entries = snapshot::read(ledger_file)?;

    let snap = Rc::new(snapshot::Snap {
        ledger_entries: ledger_entries.clone(),
    });
    let storage = Storage::with_recording_footprint(snap);
    // let contents = utils::get_contract_wasm_from_storage(&mut storage, contract_id)?;
    let h = Host::with_storage(storage);

    // let vm = Vm::new(&h, [0; 32].into(), &contents).unwrap();
    // let inputs = match contractspec::function_spec(&vm, &method) {
    //     Some(s) => s.inputs,
    //     None => {
    //         return Err(Error::FunctionNotFoundInContractSpec);
    //     }
    // };

    let mut complete_args = vec![
        ScVal::Object(Some(ScObject::Bytes(contract_id.try_into()?))),
        ScVal::Symbol(method.try_into()?),
    ];
    complete_args.extend_from_slice(params);

    let res = h.invoke_function(HostFunction::Call, complete_args.try_into()?)?;

    // TODO: Include footprint in result struct
    // TODO: Include costs in result struct
    // let cost = h.get_budget(|b| {
    //     let mut v = vec![
    //         ("cpu_insns", b.cpu_insns.get_count()),
    //         ("mem_bytes", b.mem_bytes.get_count()),
    //     ];
    //     // for cost_type in CostType::variants() {
    //     //     v.push((cost_type.try_into()?, b.get_input(*cost_type)));
    //     // }
    //     Some(v)
    // });

    // TODO: Commit here if we were "sendTransaction"
    // let (storage, _, _) = h.try_finish().map_err(|_h| {
    //     HostError::from(ScStatus::HostStorageError(
    //         ScHostStorageErrorCode::UnknownError,
    //     ))
    // })?;
    // snapshot::commit(ledger_entries, Some(&storage.map), ledger_file)?;

    Ok(json!({
        "cost": {},
        "footprint": {
            "readOnly": [],
            "readWrite": [],
        },
        "xdr": base64::encode(res.to_xdr()?),
        // TODO: Find "real" ledger seq number here
        "latestLedger": 1,
    }))
}
