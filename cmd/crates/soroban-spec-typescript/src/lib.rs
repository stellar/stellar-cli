#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]

use std::{fs, io};

use crate::types::Type;
use heck::ToLowerCamelCase;
use itertools::Itertools;
use sha2::{Digest, Sha256};
use stellar_xdr::{ScSpecEntry, WriteXdr};

use types::Entry;

use soroban_spec::read::{from_wasm, FromWasmError};

pub mod boilerplate;
mod types;
pub mod wrapper;

#[derive(thiserror::Error, Debug)]
pub enum GenerateFromFileError {
    #[error("reading file: {0}")]
    Io(io::Error),
    #[error("sha256 does not match, expected: {expected}")]
    VerifySha256 { expected: String },
    #[error("parsing contract spec: {0}")]
    Parse(stellar_xdr::Error),
    #[error("getting contract spec: {0}")]
    GetSpec(FromWasmError),
}

pub fn generate_from_file(
    file: &str,
    verify_sha256: Option<&str>,
) -> Result<String, GenerateFromFileError> {
    // Read file.
    let wasm = fs::read(file).map_err(GenerateFromFileError::Io)?;

    // Produce hash for file.
    let sha256 = Sha256::digest(&wasm);
    let sha256 = format!("{sha256:x}");

    if let Some(verify_sha256) = verify_sha256 {
        if verify_sha256 != sha256 {
            return Err(GenerateFromFileError::VerifySha256 { expected: sha256 });
        }
    }

    // Generate code.
    let json = generate_from_wasm(&wasm).map_err(GenerateFromFileError::GetSpec)?;
    Ok(json)
}

pub fn generate_from_wasm(wasm: &[u8]) -> Result<String, FromWasmError> {
    let spec = from_wasm(wasm)?;
    let json = generate(&spec);
    Ok(json)
}

fn generate_class(fns: &[Entry], spec: &[ScSpecEntry]) -> String {
    let methods = fns.iter().map(entry_to_ts).join("\n\n    ");
    let spec = spec
        .iter()
        .map(|s| format!("\"{}\"", s.to_xdr_base64().unwrap()))
        .join(",\n        ");
    format!(
        r#"export class Contract {{
            spec: ContractSpec;
    constructor(public readonly options: ClassOptions) {{
        this.spec = new ContractSpec([
            {spec}
            ]);
    }}
    {methods}
}}"#,
    )
}

pub fn generate(spec: &[ScSpecEntry]) -> String {
    let mut collected: Vec<_> = spec.iter().map(Entry::from).collect();
    if !spec.iter().any(is_error_enum) {
        collected.push(Entry::ErrorEnum {
            doc: String::new(),
            name: "Error".to_string(),
            cases: vec![],
        });
    }
    let (fns, other): (Vec<_>, Vec<_>) = collected
        .into_iter()
        .partition(|entry| matches!(entry, Entry::Function { .. }));
    let top = other.iter().map(entry_to_ts).join("\n");
    let bottom = generate_class(&fns, spec);
    format!("{top}\n\n{bottom}")
}

fn doc_to_ts_doc(doc: &str) -> String {
    if doc.is_empty() {
        String::new()
    } else {
        let doc = doc.split('\n').join("\n * ");
        format!(
            r#"/**
 * {doc}
 */
"#,
        )
    }
}

fn is_error_enum(entry: &ScSpecEntry) -> bool {
    matches!(entry, ScSpecEntry::UdtErrorEnumV0(_))
}

fn method_options(return_type: &String) -> String {
    format!(
        r#"{{
        /**
         * The fee to pay for the transaction. Default: 100.
         */
        fee?: number
        /**
         * What type of response to return.
         *
         *   - `undefined`, the default, parses the returned XDR as `{return_type}`. Runs preflight, checks to see if auth/signing is required, and sends the transaction if so. If there's no error and `secondsToWait` is positive, awaits the finalized transaction.
         *   - `'simulated'` will only simulate/preflight the transaction, even if it's a change/set method that requires auth/signing. Returns full preflight info.
         *   - `'full'` return the full RPC response, meaning either 1. the preflight info, if it's a view/read method that doesn't require auth/signing, or 2. the `sendTransaction` response, if there's a problem with sending the transaction or if you set `secondsToWait` to 0, or 3. the `getTransaction` response, if it's a change method with no `sendTransaction` errors and a positive `secondsToWait`.
         */
        responseType?: R
        /**
         * If the simulation shows that this invocation requires auth/signing, `invoke` will wait `secondsToWait` seconds for the transaction to complete before giving up and returning the incomplete {{@link SorobanClient.SorobanRpc.GetTransactionResponse}} results (or attempting to parse their probably-missing XDR with `parseResultXdr`, depending on `responseType`). Set this to `0` to skip waiting altogether, which will return you {{@link SorobanClient.SorobanRpc.SendTransactionResponse}} more quickly, before the transaction has time to be included in the ledger. Default: 10.
         */
        secondsToWait?: number
    }}"#
    )
}

fn jsify_name(name: &String) -> String {
    name.to_lower_camel_case()
}

#[allow(clippy::too_many_lines)]
pub fn entry_to_ts(entry: &Entry) -> String {
    match entry {
        Entry::Function {
            doc,
            name,
            inputs,
            outputs,
        } => {
            let input_vals = inputs.iter().map(func_input_to_arg_name).join(", ");
            let input = (!inputs.is_empty())
                .then(|| {
                    format!(
                        "{{{input_vals}}}: {{{}}}, ",
                        inputs.iter().map(func_input_to_ts).join(", ")
                    )
                })
                .unwrap_or_default();
            let mut is_result = false;
            let mut return_type: String;
            if outputs.is_empty() {
                return_type = "void".to_owned();
            } else if outputs.len() == 1 {
                return_type = type_to_ts(&outputs[0]);
                is_result = return_type.starts_with("Result<");
            } else {
                return_type = format!("readonly [{}]", outputs.iter().map(type_to_ts).join(", "));
            };
            let ts_doc = doc_to_ts_doc(doc);

            if is_result {
                return_type = return_type
                    .strip_prefix("Result<")
                    .unwrap()
                    .strip_suffix('>')
                    .unwrap()
                    .to_owned();
                return_type = format!("Ok<{return_type}> | Err<Error_> | undefined");
            }

            let mut output = outputs
                .get(0)
                .map(|_| format!("this.spec.funcResToNative(\"{name}\", xdr)"))
                .unwrap_or_default();
            if is_result {
                output = format!("new Ok({output})");
            }
            if return_type != "void" {
                output = format!(r#"return {output};"#);
            };
            let parse_result_xdr = if return_type == "void" {
                r#"parseResultXdr: () => {}"#.to_owned()
            } else {
                format!(
                    r#"parseResultXdr: (xdr): {return_type} => {{
                {output}
            }}"#
                )
            };
            let js_name = jsify_name(name);
            let options = method_options(&return_type);
            let args = format!("args: this.spec.funcArgsToScVals(\"{name}\", {{{input_vals}}}),");
            let mut body = format!(
                r#"return await invoke({{
            method: '{name}',
            {args}
            ...options,
            ...this.options,
            {parse_result_xdr},
        }});"#
            );
            if is_result {
                body = format!(
                    r#"try {{
            {body}
        }} catch (e) {{
            if (typeof e === 'string') {{
                let err = parseError(e);
                if (err) return err;
            }}
            throw e;
        }}"#
                );
            }
            format!(
                r#"{ts_doc}async {js_name}<R extends ResponseTypes = undefined>({input}options: {options} = {{}}) {{
                    {body}
    }}
"#
            )
        }
        Entry::Struct { doc, name, fields } => {
            let docs = doc_to_ts_doc(doc);
            let fields = fields.iter().map(field_to_ts).join("\n  ");
            format!(
                r#"{docs}export interface {name} {{
  {fields}
}}
"#
            )
        }

        Entry::TupleStruct { doc, name, fields } => {
            let docs = doc_to_ts_doc(doc);
            let fields = fields.iter().map(type_to_ts).join(",  ");
            format!("{docs}export type {name} = readonly [{fields}];")
        }

        Entry::Union { name, doc, cases } => {
            let doc = doc_to_ts_doc(doc);
            let cases = cases.iter().map(case_to_ts).join(" | ");

            format!(
                r#"{doc}export type {name} = {cases};
"#
            )
        }
        Entry::Enum { doc, name, cases } => {
            let doc = doc_to_ts_doc(doc);
            let cases = cases.iter().map(enum_case_to_ts).join("\n  ");
            let name = (name == "Error")
                .then(|| format!("{name}s"))
                .unwrap_or(name.to_string());
            format!(
                r#"{doc}export enum {name} {{
  {cases}
}}
"#,
            )
        }
        Entry::ErrorEnum { doc, cases, .. } => {
            let doc = doc_to_ts_doc(doc);
            let cases = cases
                .iter()
                .map(|c| format!("{}: {{message:\"{}\"}}", c.value, c.doc))
                .join(",\n  ");
            format!(
                r#"{doc}const Errors = {{
{cases}
}}"#
            )
        }
    }
}

fn enum_case_to_ts(case: &types::EnumCase) -> String {
    let types::EnumCase { name, value, .. } = case;
    format!("{name} = {value},")
}

fn case_to_ts(case: &types::UnionCase) -> String {
    let types::UnionCase { name, values, .. } = case;
    format!(
        "{{tag: \"{name}\", values: {}}}",
        type_to_ts(&Type::Tuple {
            elements: values.clone(),
        })
    )
}

fn field_to_ts(field: &types::StructField) -> String {
    let types::StructField { doc, name, value } = field;
    let doc = doc_to_ts_doc(doc);
    let type_ = type_to_ts(value);
    format!("{doc}{name}: {type_};")
}

pub fn func_input_to_ts(input: &types::FunctionInput) -> String {
    let types::FunctionInput { name, value, .. } = input;
    let type_ = type_to_ts(value);
    format!("{name}: {type_}")
}

pub fn func_input_to_arg_name(input: &types::FunctionInput) -> String {
    let types::FunctionInput { name, .. } = input;
    name.to_string()
}

pub fn type_to_ts(value: &types::Type) -> String {
    match value {
        types::Type::U64 => "u64".to_owned(),
        types::Type::I64 => "i64".to_owned(),
        types::Type::U128 => "u128".to_owned(),
        types::Type::I128 => "i128".to_owned(),
        types::Type::U32 => "u32".to_owned(),
        types::Type::I32 => "i32".to_owned(),
        types::Type::Bool => "boolean".to_owned(),
        types::Type::Symbol | types::Type::String => "string".to_owned(),
        types::Type::Map { key, value } => {
            format!("Map<{}, {}>", type_to_ts(key), type_to_ts(value))
        }
        types::Type::Option { value } => format!("Option<{}>", type_to_ts(value)),
        types::Type::Result { value, .. } => {
            format!("Result<{}>", type_to_ts(value))
        }
        types::Type::Set { element } => format!("Set<{}>", type_to_ts(element)),
        types::Type::Vec { element } => format!("Array<{}>", type_to_ts(element)),
        types::Type::Tuple { elements } => {
            if elements.is_empty() {
                "void".to_owned()
            } else {
                format!("readonly [{}]", elements.iter().map(type_to_ts).join(", "))
            }
        }
        types::Type::Custom { name } => name.clone(),
        // TODO: Figure out what js type to map this to. There is already an `Error_` one that
        // ahalabs have added in the bindings, so.. maybe rename that?
        types::Type::Val => "any".to_owned(),
        types::Type::Error { .. } => "Error_".to_owned(),
        types::Type::Address => "Address".to_string(),
        types::Type::Bytes | types::Type::BytesN { .. } => "Buffer".to_string(),
        types::Type::Void => "void".to_owned(),
        types::Type::U256 => "u256".to_string(),
        types::Type::I256 => "i256".to_string(),
        types::Type::Timepoint => "Timepoint".to_string(),
        types::Type::Duration => "Duration".to_string(),
    }
}
