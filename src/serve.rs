use std::{fmt::Debug, fs, io, io::Cursor, sync::Arc, rc::Rc, path::PathBuf};

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

use crate::snapshot;
use crate::invoke;
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
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(tag = "method", content = "params")]
#[serde(rename_all = "snake_case")]
enum Requests {
    Call { id: String, func: String, args: Option<Vec<Value>>, args_xdr: Option<Vec<String>> },
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(tag = "method", content = "params")]
#[serde(rename_all = "snake_case")]
enum Notifications {
}

#[derive(Debug)]
enum JsonRpc<N, R> {
    Request(usize, R),
    Notification(N),
}

impl<N, R> serde::Serialize for JsonRpc<N, R>
    where N: serde::Serialize,
          R: serde::Serialize
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        match *self {
            JsonRpc::Request(id, ref r) => {
                let mut v = serde_json::to_value(r).map_err(serde::ser::Error::custom)?;
                v["id"] = json!(id);
                v.serialize(serializer)
            }
            JsonRpc::Notification(ref n) => n.serialize(serializer),
        }
    }
}

impl<'de, N, R> serde::Deserialize<'de> for JsonRpc<N, R>
    where N: serde::Deserialize<'de>,
          R: serde::Deserialize<'de>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: serde::Deserializer<'de>
    {
        #[derive(serde::Deserialize)]
        struct IdHelper {
            id: Option<usize>,
        }

        let v = Value::deserialize(deserializer)?;
        let helper = IdHelper::deserialize(&v).map_err(serde::de::Error::custom)?;
        match helper.id {
            Some(id) => {
                let r = R::deserialize(v).map_err(serde::de::Error::custom)?;
                Ok(JsonRpc::Request(id, r))
            }
            None => {
                let n = N::deserialize(v).map_err(serde::de::Error::custom)?;
                Ok(JsonRpc::Notification(n))
            }
        }
    }
}

type Request = JsonRpc<Notifications, Requests>;

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
            .map(|request: Requests, ledger_file: Arc<PathBuf>| match request {
                Requests::Call { id, func, args, args_xdr } => {
                    let lf = ledger_file.clone();
                    reply(invoke(id, func, args.unwrap_or_default(), args_xdr.unwrap_or_default(), &lf))
                }
            });

        warp::serve(call)
            .run(([127, 0, 0, 1], self.port.unwrap_or(8080)))
            .await;
        Ok(())
    }
}

fn reply(result: Result<ScVal, Error>) -> impl warp::Reply {
    match result {
        Ok(res) => {
            let mut ret_xdr_buf: Vec<u8> = Vec::new();
            match (strval::to_string(&res), res.write_xdr(&mut Cursor::new(&mut ret_xdr_buf))) {
                (Ok(j), Ok(())) => json!({
                    "result": {
                        "json": j,
                        "xdr": base64::encode(ret_xdr_buf),
                    },
                }).to_string(),
                (Err(err), _) => reply(Err(Error::StrVal(err))),
                (_, Err(err)) => reply(Err(Error::Xdr(err))),
            }
        }
        Err(err) => {
            json!({ "error": err.to_string() }).to_string()
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
