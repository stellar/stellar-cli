use std::{
    env, fs,
    io::{stdin, Cursor, Write},
    path::PathBuf,
    process::{self, Stdio},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use stellar_xdr::curr;

use crate::{commands::global, print::Print};

const DEFAULT_JSON: &str = r#"{
  "tx": {
    "tx": {
      "source_account": "",
      "fee": 100,
      "seq_num": 0,
      "cond": "none",
      "memo": "none",
      "operations": [],
      "ext": "v0"
    },
    "signatures": []
  }
}
"#;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    StellarXdr(#[from] stellar_xdr::curr::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    Base64Decode(#[from] base64::DecodeError),

    #[error("Editor returned non-zero status")]
    EditorNonZeroStatus,
}

// Command to edit the transaction
/// e.g. `stellar tx new manage-data --data-name hello --build-only | stellar tx edit`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let json: String = if atty::isnt(atty::Stream::Stdin) {
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            let input = input.trim();
            xdr_to_json::<curr::TransactionEnvelope>(input)?
        } else {
            DEFAULT_JSON.to_string()
        };

        let path = tmp_file(&json)?;
        open_editor(&print, &path)?;

        let contents = fs::read_to_string(&path)?;
        let xdr = json_to_xdr::<curr::TransactionEnvelope>(&contents)?;
        fs::remove_file(&path)?;

        println!("{xdr}");

        Ok(())
    }
}

fn tmp_file(contents: &str) -> Result<PathBuf, Error> {
    let temp_dir = env::temp_dir();
    let file_name = format!("stellar-json-xdr-{}.json", rand::random::<u64>());
    let path = temp_dir.join(file_name);

    let mut file = fs::File::create(&path)?;
    file.write_all(contents.as_bytes())?;

    Ok(path)
}

fn get_editor() -> String {
    env::var("STELLAR_EDITOR")
        .or_else(|_| env::var("EDITOR"))
        .or_else(|_| env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string())
}

fn open_editor(print: &Print, path: &PathBuf) -> Result<(), Error> {
    let editor = get_editor();
    print.infoln(format!("Using `{editor}`"));

    let parts: Vec<&str> = editor.split_whitespace().collect();
    let cmd = parts[0];
    let args = &parts[1..];

    print.infoln("Opening editor to edit the transaction envelope...".to_string());

    let result = process::Command::new(cmd)
        .stdin(Stdio::null())
        .args(args)
        .arg(path)
        .spawn()?
        .wait_with_output()?;

    if result.status.success() {
        Ok(())
    } else {
        Err(Error::EditorNonZeroStatus)
    }
}

fn xdr_to_json<T>(xdr_string: &str) -> Result<String, Error>
where
    T: curr::ReadXdr + serde::Serialize,
{
    let xdr_bytes = BASE64.decode(xdr_string)?;
    let cursor = Cursor::new(xdr_bytes);
    let mut limit = curr::Limited::new(cursor, curr::Limits::none());
    let value = T::read_xdr(&mut limit)?;
    let json = serde_json::to_string_pretty(&value)?;

    Ok(json)
}

fn json_to_xdr<T>(json_string: &str) -> Result<String, Error>
where
    T: serde::de::DeserializeOwned + curr::WriteXdr,
{
    let value: T = serde_json::from_str(json_string)?;
    let mut data = Vec::new();
    let cursor = Cursor::new(&mut data);
    let mut limit = curr::Limited::new(cursor, curr::Limits::none());
    value.write_xdr(&mut limit)?;

    let xdr_base64 = BASE64.encode(&data);
    Ok(xdr_base64)
}
