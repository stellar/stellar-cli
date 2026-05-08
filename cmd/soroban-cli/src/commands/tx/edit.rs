use std::{
    env,
    fs::{self},
    io::{stdin, Cursor, IsTerminal},
    path::PathBuf,
    process::{self},
};

use tempfile::TempDir;

use serde_json::json;
use stellar_xdr::curr;

use crate::{commands::global, print::Print};

fn schema_url() -> String {
    let ver = stellar_xdr::VERSION.pkg;
    format!("https://stellar.org/schema/xdr-json/v{ver}/TransactionEnvelope.json")
}

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
        let json: String = if stdin().is_terminal() {
            default_json()
        } else {
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            let input = input.trim();
            xdr_to_json::<curr::TransactionEnvelope>(input)?
        };

        let (_temp_dir, path) = tmp_file(&json)?;
        let editor = get_editor();

        print.infoln(format!("Editing transaction at {}", path.display()));
        open_editor(&print, &editor, &path)?;

        let contents = fs::read_to_string(&path)?;
        let xdr = json_to_xdr::<curr::TransactionEnvelope>(&contents)?;

        println!("{xdr}");

        Ok(())
    }
}

struct Editor {
    cmd: String,
    source: String,
    args: Vec<String>,
}

fn tmp_file(contents: &str) -> Result<(TempDir, PathBuf), Error> {
    let temp_dir = tempfile::Builder::new()
        .prefix("stellar-tx-edit-")
        .tempdir()?;
    let path = temp_dir.path().join("edit.json");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(temp_dir.path(), fs::Permissions::from_mode(0o700))?;
    }

    crate::config::locator::write_hardened_file(&path, contents.as_bytes())?;

    Ok((temp_dir, path))
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

    // Windows doesn't have devices like /dev/tty.
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
    schema["$schema"] = json!(schema_url());
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

fn default_json() -> String {
    let schema_url = schema_url();
    format!(
        r#"{{
  "$schema": "{schema_url}",
  "tx": {{
    "tx": {{
      "source_account": "",
      "fee": 100,
      "seq_num": 0,
      "cond": "none",
      "memo": "none",
      "operations": [],
      "ext": "v0"
    }},
    "signatures": []
  }}
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmp_file_uses_private_tempdir() {
        let contents = r#"{"test": true}"#;
        let (temp_dir, path) = tmp_file(contents).expect("tmp_file failed");

        // File must exist inside the tempdir, not in CWD
        assert!(path.starts_with(temp_dir.path()));
        assert_ne!(temp_dir.path(), env::current_dir().unwrap());

        // Contents must match
        let read_back = fs::read_to_string(&path).expect("read failed");
        assert_eq!(read_back, contents);
    }

    #[cfg(unix)]
    #[test]
    fn tmp_file_has_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let (temp_dir, path) = tmp_file("{}").expect("tmp_file failed");

        let file_meta = fs::metadata(&path).expect("file metadata failed");
        let file_mode = file_meta.permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "file permissions should be 0o600");

        let dir_meta = fs::metadata(temp_dir.path()).expect("dir metadata failed");
        let dir_mode = dir_meta.permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "tempdir permissions should be 0o700");
    }
}
