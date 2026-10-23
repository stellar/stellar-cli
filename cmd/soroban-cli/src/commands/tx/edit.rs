use std::{
    collections::HashMap,
    env,
    fs::{self},
    io::{stdin, Cursor, IsTerminal},
    path::PathBuf,
    process::{self},
};

use serde_json::json;
use soroban_spec_tools::contract::Spec;
use stellar_xdr::ScSpecEntry;
use tempfile::TempDir;

use crate::{commands::global, print::Print, utils::XDR_DEPTH_LIMIT, wasm};

fn schema_url() -> String {
    let ver = stellar_xdr::VERSION.pkg;
    format!("https://stellar.org/schema/xdr-json/v{ver}/TransactionEnvelope.json")
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    StellarXdr(#[from] stellar_xdr::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    Base64Decode(#[from] base64::DecodeError),

    #[error(transparent)]
    Wasm(#[from] wasm::Error),

    #[error("Editor returned non-zero status")]
    EditorNonZeroStatus,
}

// Command to edit the transaction
/// e.g. `stellar tx new manage-data --data-name hello --build-only | stellar tx edit`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    /// Optional path(s) to contract WASM file(s) used to annotate `invoke_contract` arguments with parameter names in JSONC comments
    #[arg(long)]
    pub wasm: Vec<PathBuf>,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let fn_params = self.load_fn_param_names()?;
        let json: String = if stdin().is_terminal() {
            default_json()
        } else {
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            let input = input.trim();
            xdr_to_json_with_specs::<stellar_xdr::TransactionEnvelope>(input, &fn_params)?
        };

        let (_temp_dir, path) = tmp_file(&json)?;
        let editor = get_editor();

        print.infoln(format!("Editing transaction at {}", path.display()));
        open_editor(&print, &editor, &path)?;

        let contents = fs::read_to_string(&path)?;
        let xdr = json_to_xdr::<stellar_xdr::TransactionEnvelope>(&contents)?;

        println!("{xdr}");

        Ok(())
    }

    fn load_fn_param_names(&self) -> Result<HashMap<String, Vec<String>>, Error> {
        let mut map = HashMap::new();
        for wasm_path in &self.wasm {
            let wasm_args = wasm::Args {
                wasm: wasm_path.clone(),
            };
            let Spec { spec, .. } = wasm_args.parse()?;
            for (fn_name, params) in extract_fn_param_names(&spec) {
                map.insert(fn_name, params);
            }
        }
        Ok(map)
    }
}

pub fn extract_fn_param_names(entries: &[ScSpecEntry]) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    for entry in entries {
        if let ScSpecEntry::FunctionV0(func) = entry {
            let fn_name = func.name.to_utf8_string_lossy();
            let params = func
                .inputs
                .iter()
                .map(|input| input.name.to_utf8_string_lossy())
                .collect::<Vec<_>>();
            map.insert(fn_name, params);
        }
    }
    map
}

pub fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < len {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
        } else if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
        } else if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }

    out
}

pub fn annotate_invoke_contract_args(
    json: &str,
    fn_params: &HashMap<String, Vec<String>>,
) -> String {
    if fn_params.is_empty() {
        return json.to_string();
    }

    let lines: Vec<&str> = json.lines().collect();
    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len());
    let mut current_fn: Option<String> = None;
    let mut in_args = false;
    let mut args_depth: i32 = 0;
    let mut arg_index: usize = 0;

    for line in lines {
        let trimmed = line.trim();

        if !in_args {
            if let Some(rest) = trimmed.strip_prefix("\"function_name\":") {
                let val = rest.trim().trim_end_matches(',').trim();
                if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                    current_fn = Some(val[1..val.len() - 1].to_string());
                }
            } else if trimmed.starts_with("\"args\": [") && current_fn.is_some() {
                if !trimmed.ends_with(']') && !trimmed.ends_with("],") {
                    in_args = true;
                    args_depth = 0;
                    arg_index = 0;
                } else {
                    current_fn = None;
                }
            }
            out_lines.push(line.to_string());
            continue;
        }

        // Inside `"args": [`
        if args_depth == 0 && (trimmed == "]" || trimmed == "],") {
            in_args = false;
            current_fn = None;
            out_lines.push(line.to_string());
            continue;
        }

        let delta = bracket_delta(trimmed);
        let prev_depth = args_depth;
        args_depth += delta;

        let finished_top_level_arg = prev_depth >= 0 && args_depth == 0;

        if finished_top_level_arg {
            let comment = current_fn
                .as_ref()
                .and_then(|fn_name| fn_params.get(fn_name))
                .and_then(|params| params.get(arg_index));
            arg_index += 1;

            if let Some(param_name) = comment {
                out_lines.push(format!("{line} // {param_name}"));
            } else {
                out_lines.push(line.to_string());
            }
        } else {
            out_lines.push(line.to_string());
        }
    }

    let mut result = out_lines.join("\n");
    if json.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn bracket_delta(line: &str) -> i32 {
    let mut delta = 0i32;
    let mut in_string = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
        } else {
            match c {
                '"' => in_string = true,
                '{' | '[' => delta += 1,
                '}' | ']' => delta -= 1,
                _ => {}
            }
        }
        i += 1;
    }
    delta
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

pub fn xdr_to_json_with_specs<T>(
    xdr_string: &str,
    fn_params: &HashMap<String, Vec<String>>,
) -> Result<String, Error>
where
    T: stellar_xdr::ReadXdr + serde::Serialize,
{
    let tx = T::from_xdr_base64(xdr_string, stellar_xdr::Limits::depth(XDR_DEPTH_LIMIT))?;
    let mut schema: serde_json::Value = serde_json::to_value(tx)?;
    schema["$schema"] = json!(schema_url());
    let json = serde_json::to_string_pretty(&schema)?;

    Ok(annotate_invoke_contract_args(&json, fn_params))
}

fn json_to_xdr<T>(json_string: &str) -> Result<String, Error>
where
    T: serde::de::DeserializeOwned + stellar_xdr::WriteXdr,
{
    let stripped = strip_jsonc_comments(json_string);
    let mut schema: serde_json::Value = serde_json::from_str(&stripped)?;

    if let Some(obj) = schema.as_object_mut() {
        obj.remove("$schema");
    }

    let json_string = serde_json::to_string(&schema)?;

    let value: T = serde_json::from_str(json_string.as_str())?;
    let mut data = Vec::new();
    let cursor = Cursor::new(&mut data);
    let mut limit = stellar_xdr::Limited::new(cursor, stellar_xdr::Limits::depth(XDR_DEPTH_LIMIT));
    value.write_xdr(&mut limit)?;

    Ok(value.to_xdr_base64(stellar_xdr::Limits::depth(XDR_DEPTH_LIMIT))?)
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
    use stellar_xdr::{ScSpecFunctionInputV0, ScSpecFunctionV0, ScSpecTypeDef, StringM, VecM};

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

    #[test]
    fn strip_jsonc_comments_preserves_urls_and_removes_comments() {
        let jsonc = r#"{
  "$schema": "https://stellar.org/schema/xdr-json/v23/TransactionEnvelope.json", // schema url
  /* block comment */
  "value": 42 // answer
}"#;
        let stripped = strip_jsonc_comments(jsonc);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(
            parsed["$schema"],
            "https://stellar.org/schema/xdr-json/v23/TransactionEnvelope.json"
        );
        assert_eq!(parsed["value"], 42);
    }

    #[test]
    fn annotate_invoke_contract_args_injects_param_names() {
        let spec_entries = vec![ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
            doc: StringM::default(),
            name: "add_liquidity".try_into().unwrap(),
            inputs: VecM::try_from(vec![
                ScSpecFunctionInputV0 {
                    doc: StringM::default(),
                    name: "token_a".try_into().unwrap(),
                    type_: ScSpecTypeDef::Address,
                },
                ScSpecFunctionInputV0 {
                    doc: StringM::default(),
                    name: "amount_a".try_into().unwrap(),
                    type_: ScSpecTypeDef::I128,
                },
            ])
            .unwrap(),
            outputs: VecM::default(),
        })];

        let fn_params = extract_fn_param_names(&spec_entries);
        let raw_json = r#"{
  "invoke_contract": {
    "contract_address": "CAG5LRYQ5JVEUI5TEID72EYOVX44TTUJT5BQR2J6J77FH65PCCFAJDDH",
    "function_name": "add_liquidity",
    "args": [
      {
        "address": "CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"
      },
      {
        "i128": "1000000000"
      }
    ]
  }
}"#;

        let annotated = annotate_invoke_contract_args(raw_json, &fn_params);
        assert!(annotated.contains("}, // token_a"));
        assert!(annotated.contains("} // amount_a"));

        let stripped = strip_jsonc_comments(&annotated);
        let reparsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(
            reparsed["invoke_contract"]["function_name"],
            "add_liquidity"
        );
    }
}
