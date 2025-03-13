use std::{
    env,
    fs::{self},
    io::{stdin, Cursor, IsTerminal, Write},
    path::PathBuf,
    process::{self},
};

use serde_json::json;
use stellar_xdr::curr;

use crate::{commands::global, print::Print};

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

    #[error("No stdin detected")]
    NoStdin,
}

// Command to edit the transaction
/// e.g. `stellar tx new manage-data --data-name hello --build-only | stellar tx edit`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let json: String = if stdin().is_terminal() {
            return Err(Error::NoStdin);
        } else {
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            let input = input.trim();
            xdr_to_json::<curr::TransactionEnvelope>(input)?
        };

        let path = tmp_file(&json)?;
        let editor = get_editor();

        open_editor(&print, &editor, &path)?;

        let contents = fs::read_to_string(&path)?;
        let xdr = json_to_xdr::<curr::TransactionEnvelope>(&contents)?;
        fs::remove_file(&path)?;

        println!("{xdr}");

        Ok(())
    }
}

struct Editor {
    cmd: String,
    source: String,
    args: Vec<String>,
}

fn tmp_file(contents: &str) -> Result<PathBuf, Error> {
    let temp_dir = env::current_dir().unwrap_or(env::temp_dir());
    let file_name = format!("stellar-xdr-{}.json", rand::random::<u64>());
    let path = temp_dir.join(file_name);

    let mut file = fs::File::create(&path)?;
    file.write_all(contents.as_bytes())?;

    Ok(path)
}

fn get_editor() -> Editor {
    let (source, cmd) = env::var("STELLAR_EDITOR")
        .map(|val| ("STELLAR_EDITOR", val))
        .or_else(|_| env::var("EDITOR").map(|val| ("EDITOR", val)))
        .or_else(|_| env::var("VISUAL").map(|val| ("VISUAL", val)))
        .unwrap_or_else(|_| ("default", "vim".to_string()));

    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let cmd = parts[0].to_string();
    let args = &parts[1..]
        .iter()
        .map(|&s| s.to_string())
        .collect::<Vec<String>>();

    Editor {
        source: source.to_string(),
        cmd,
        args: args.clone(),
    }
}

fn open_editor(print: &Print, editor: &Editor, path: &PathBuf) -> Result<(), Error> {
    print.infoln(format!(
        "Opening editor with `{source}=\"{cmd}\"`...",
        source = editor.source,
        cmd = editor.cmd,
    ));

    let mut binding = process::Command::new(editor.cmd.clone());
    let command = binding.args(editor.args.clone()).arg(path);

    #[cfg(unix)]
    {
        use fs::File;
        let tty = File::open("/dev/tty")?;
        let tty_out = fs::OpenOptions::new().write(true).open("/dev/tty")?;
        let tty_err = fs::OpenOptions::new().write(true).open("/dev/tty")?;

        command
            .stdin(tty)
            .stdout(tty_out)
            .stderr(tty_err)
            .env("TERM", "xterm-256color");
    }

    let status = command.spawn()?.wait()?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::EditorNonZeroStatus)
    }
}

fn xdr_to_json<T>(xdr_string: &str) -> Result<String, Error>
where
    T: curr::ReadXdr + serde::Serialize,
{
    let tx = T::from_xdr_base64(xdr_string, curr::Limits::none())?;
    let mut schema: serde_json::Value = serde_json::to_value(tx)?;
    schema["$schema"] = json!(
        "https://github.com/stellar/rs-stellar-xdr/raw/main/xdr-json/curr/TransactionEnvelope.json"
    );
    let json = serde_json::to_string_pretty(&schema)?;

    Ok(json)
}

fn json_to_xdr<T>(json_string: &str) -> Result<String, Error>
where
    T: serde::de::DeserializeOwned + curr::WriteXdr,
{
    let mut schema: serde_json::Value = serde_json::from_str(json_string)?;

    if let Some(obj) = schema.as_object_mut() {
        obj.remove("$schema");
    }

    let json_string = serde_json::to_string(&schema)?;

    let value: T = serde_json::from_str(json_string.as_str())?;
    let mut data = Vec::new();
    let cursor = Cursor::new(&mut data);
    let mut limit = curr::Limited::new(cursor, curr::Limits::none());
    value.write_xdr(&mut limit)?;

    Ok(value.to_xdr_base64(curr::Limits::none())?)
}
