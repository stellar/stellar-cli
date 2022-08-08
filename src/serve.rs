use std::{
    fmt::Debug,
    fs,
    io,
    io::Cursor,
    net::SocketAddr,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};

use clap::Parser;
use serde_json::{Value, json};
use soroban_env_host::{
    budget::CostType,
    storage::Storage,
    xdr::{
        Error as XdrError, HostFunction, ReadXdr, WriteXdr, ScHostStorageErrorCode, ScObject, ScSpecEntry,
        ScSpecFunctionV0, ScStatus, ScVal,
    },
    Host, HostError, Vm,
};
use warp::Filter;
use hex::FromHexError;

use crate::invoke;
use crate::jsonrpc;
use crate::snapshot;
use crate::strval::{self, StrValError};
use crate::utils;

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file to deploy to the contract ID and invoke
    #[clap(long)]
    port: Option<u16>,
    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value("ledger.json"))]
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
    Call { id: String, func: String, args: Option<Vec<Value>>, args_xdr: Option<Vec<String>> },
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        // let context = HostContext::default();
        // eprintln!("Hello World!");
        // process::exit(0);

        let ledger_file = Arc::new(self.ledger_file.clone());
        let with_ledger_file = warp::any().map(move || ledger_file.clone());

        let call = warp::post()
            .and(warp::path("rpc"))
            .and(warp::body::json())
            .and(with_ledger_file)
            .map(|request: jsonrpc::Request<Requests>, ledger_file: Arc<PathBuf>| {
                if request.jsonrpc != "2.0" {
                    return json!({
                        "jsonrpc": "2.0",
                        "id": &request.id,
                        "error": {
                            "code":-32600,
                            "message": "Invalid jsonrpc value in request",
                        },
                    }).to_string();
                }
                let result = match (request.method.as_str(), request.params) {
                    ("call", Some(Requests::Call { id, func, args, args_xdr })) => {
                        let lf = ledger_file.clone();
                        invoke(id, func, args.unwrap_or_default(), args_xdr.unwrap_or_default(), &lf)
                    },
                    _ => Err(Error::UnknownMethod),
                };
                let r = reply(&request.id, result);
                serde_json::to_string(&r).unwrap_or(json!({
                    "jsonrpc": "2.0",
                    "id": &request.id,
                    "error": {
                    "code":-32603,
                    "message": "Internal server error",
                    },
                }).to_string())
            });

        let addr: SocketAddr = ([127, 0, 0, 1], self.port.unwrap_or(8080)).into();
        println!("Listening on: {}", addr);
        warp::serve(call)
            .run(addr)
            .await;
        Ok(())
    }
}

fn reply(id: &Option<jsonrpc::Id>, result: Result<ScVal, Error>) -> jsonrpc::Response<Value, Value> {
    match result {
        Ok(res) => {
            let mut ret_xdr_buf: Vec<u8> = Vec::new();
            match (strval::to_string(&res), res.write_xdr(&mut Cursor::new(&mut ret_xdr_buf))) {
                (Ok(j), Ok(())) => jsonrpc::Response::Ok(jsonrpc::ResultResponse{
                    jsonrpc: "2.0".to_string(),
                    id: id.as_ref().unwrap_or(&jsonrpc::Id::Null).clone(),
                    result: json!({
                        "json": j,
                        "xdr": base64::encode(ret_xdr_buf),
                    })
                }),
                (Err(err), _) => reply(id, Err(Error::StrVal(err))),
                (_, Err(err)) => reply(id, Err(Error::Xdr(err))),
            }
        }
        Err(err) => {
            jsonrpc::Response::Err(jsonrpc::ErrorResponse{
                jsonrpc: "2.0".to_string(),
                id: id.as_ref().unwrap_or(&jsonrpc::Id::Null).clone(),
                error: jsonrpc::ErrorResponseError{
                    code: match err {
                        Error::Serde(_) => -32700,
                        Error::UnknownMethod => -32601,
                        _ => -32603
                    },
                    message: err.to_string(),
                    data: None,
                },
            })
        }
    }
}

fn invoke(contract: String, func: String, args: Vec<Value>, args_xdr: Vec<String>, ledger_file: &PathBuf) -> Result<ScVal, Error> {
    let contract_id: [u8; 32] = utils::contract_id_from_str(&contract)?;

    // Initialize storage and host
    // TODO: allow option to separate input and output file
    let ledger_entries = snapshot::read(ledger_file)?;

    let snap = Rc::new(snapshot::Snap {
        ledger_entries: ledger_entries.clone(),
    });
    let mut storage = Storage::with_recording_footprint(snap);
    let contents = utils::get_contract_wasm_from_storage(&mut storage, contract_id)?;
    let h = Host::with_storage(storage);

    let vm = Vm::new(&h, [0; 32].into(), &contents).unwrap();
    let input_types = match invoke::Cmd::function_spec(&vm, &func) {
        Some(s) => s.input_types,
        None => {
            return Err(Error::FunctionNotFoundInContractSpec);
        }
    };

    // re-assemble the args, to match the order given on the command line
    let args: Vec<ScVal> = if args_xdr.is_empty() {
        args
            .iter()
            .zip(input_types.iter())
            .map(|(a, t)| strval::from_json(a, t))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        args_xdr
            .iter()
            .map(|a| match base64::decode(a) {
                Err(_) => Err(StrValError::InvalidValue),
                Ok(b) => ScVal::from_xdr(b).map_err(StrValError::Xdr),
            })
            .collect::<Result<Vec<_>, _>>()?
    };


    let mut complete_args = vec![
        ScVal::Object(Some(ScObject::Binary(contract_id.try_into()?))),
        ScVal::Symbol((&func).try_into()?),
    ];
    complete_args.extend_from_slice(args.as_slice());

    let res = h.invoke_function(HostFunction::Call, complete_args.try_into()?)?;

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

    let storage = h.recover_storage().map_err(|_h| {
        HostError::from(ScStatus::HostStorageError(
            ScHostStorageErrorCode::UnknownError,
        ))
    })?;

    snapshot::commit(ledger_entries, Some(&storage.map), ledger_file)?;

    Ok(res)
}
