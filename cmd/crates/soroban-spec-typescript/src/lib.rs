#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]

use std::{fs, io};

use crate::types::{StructField, Type, UnionCase};
use heck::ToLowerCamelCase;
use itertools::Itertools;
use sha2::{Digest, Sha256};
use stellar_xdr::ScSpecEntry;

use types::Entry;

use crate::wrapper::type_to_js_xdr;
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

pub fn generate(spec: &[ScSpecEntry]) -> String {
    let mut collected: Vec<_> = spec.iter().map(Entry::from).collect();
    if !spec.iter().any(is_error_enum) {
        collected.push(Entry::ErrorEnum {
            doc: String::new(),
            name: "Error".to_string(),
            cases: vec![],
        });
    }
    collected.iter().map(entry_to_ts).join("\n")
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

#[allow(clippy::too_many_lines)]
pub fn entry_to_ts(entry: &Entry) -> String {
    match entry {
        Entry::Function {
            doc,
            name,
            inputs,
            outputs,
        } => {
            let args = inputs
                .iter()
                .map(|i| format!("((i) => {})({})", type_to_js_xdr(&i.value), i.name))
                .join(",\n        ");
            let input = (!inputs.is_empty())
                .then(|| {
                    format!(
                        "{{{}}}: {{{}}},",
                        inputs.iter().map(func_input_to_arg_name).join(", "),
                        inputs.iter().map(func_input_to_ts).join(", ")
                    )
                })
                .unwrap_or_default();
            let mut is_result = false;
            let mut inner_return_type = String::new();
            let return_type = if outputs.is_empty() {
                ": Promise<void>".to_owned()
            } else if outputs.len() == 1 {
                inner_return_type = type_to_ts(&outputs[0]);
                is_result = inner_return_type.starts_with("Result<");
                format!(": Promise<{inner_return_type}>")
            } else {
                format!(
                    ": Promise<[{}]>>",
                    outputs.iter().map(type_to_ts).join(", ")
                )
            };
            let ts_doc = doc_to_ts_doc(doc);

            // let output_parser = outputs.get(0).map(scVal_to_type).unwrap_or_default();
            if is_result {
                inner_return_type = inner_return_type
                    .strip_prefix("Result<")
                    .unwrap()
                    .strip_suffix('>')
                    .unwrap()
                    .to_owned();
            }

            let mut output = outputs
                .get(0)
                .map(|type_| {
                    if let Type::Custom { name } = type_ {
                        format!("{name}FromXdr(response.xdr)")
                    } else {
                        format!("scValStrToJs(response.xdr) as {inner_return_type}")
                    }
                })
                .unwrap_or_default();
            if is_result {
                output = format!("new Ok({output})");
            }
            let mut output = format!(
                r#"
    // @ts-ignore Type does exist
    const response = await invoke(invokeArgs);
    return {output};"#
            );
            if is_result {
                output = format!(
                    r#"
    try {{
        {output}
    }} catch (e) {{
        //@ts-ignore
        let err = getError(e.message);
        if (err) {{
            return err;
        }} else {{
            throw e;
        }}
    }}"#
                );
            }
            let args = (!inputs.is_empty())
                .then(|| format!("args: [{args}], "))
                .unwrap_or_default();
            format!(
                r#"{ts_doc}export async function {name}({input} {{signAndSend, fee}}: {{signAndSend?: boolean, fee?: number}} = {{signAndSend: false, fee: 100}}){return_type} {{
    let invokeArgs: InvokeArgs = {{
        signAndSend,
        fee,
        method: '{name}', 
        {args}
    }};
    {output}
}}
"#
            )
        }
        Entry::Struct { doc, name, fields } => {
            let docs = doc_to_ts_doc(doc);
            let arg_name = name.to_lower_camel_case();
            let encoded_fields = js_to_xdr_fields(&arg_name, fields);
            let decoded_fields = js_from_xdr_fields(fields);
            let fields = fields.iter().map(field_to_ts).join("\n  ");
            let void = type_to_js_xdr(&Type::Void);
            format!(
                r#"{docs}export interface {name} {{
  {fields}
}}

function {name}ToXdr({arg_name}?: {name}): xdr.ScVal {{
    if (!{arg_name}) {{
        return {void};
    }}
    let arr = [
        {encoded_fields}
        ];
    return xdr.ScVal.scvMap(arr);
}}


function {name}FromXdr(base64Xdr: string): {name} {{
    let scVal = strToScVal(base64Xdr);
    let obj: [string, any][] = scVal.map()!.map(e => [e.key().str() as string, e.val()]);
    let map = new Map<string, any>(obj);
    if (!obj) {{
        throw new Error('Invalid XDR');
    }}
    return {{
        {decoded_fields}
    }};
}}
"#
            )
        }

        Entry::TupleStruct { doc, name, fields } => {
            let docs = doc_to_ts_doc(doc);
            let arg_name = name.to_lower_camel_case();
            let encoded_fields = fields
                .iter()
                .enumerate()
                .map(|(i, t)| format!("(i => {})({arg_name}[{i}])", type_to_js_xdr(t),))
                .join(",\n        ");
            let fields = fields.iter().map(type_to_ts).join(",  ");
            let void = type_to_js_xdr(&Type::Void);
            format!(
                r#"{docs}export type {name} = [{fields}];

function {name}ToXdr({arg_name}?: {name}): xdr.ScVal {{
    if (!{arg_name}) {{
        return {void};
    }}
    let arr = [
        {encoded_fields}
        ];
    return xdr.ScVal.scvVec(arr);
}}


function {name}FromXdr(base64Xdr: string): {name} {{
    return scValStrToJs(base64Xdr) as {name};
}}
"#
            )
        }

        Entry::Union { name, doc, cases } => {
            let doc = doc_to_ts_doc(doc);
            let arg_name = name.to_lower_camel_case();
            let encoded_cases = js_to_xdr_union_cases(&arg_name, cases);
            let cases = cases.iter().map(case_to_ts).join(" | ");
            let void = type_to_js_xdr(&Type::Void);

            format!(
                r#"{doc}export type {name} = {cases};

function {name}ToXdr({arg_name}?: {name}): xdr.ScVal {{
    if (!{arg_name}) {{
        return {void};
    }}
    let res: xdr.ScVal[] = [];
    switch ({arg_name}.tag) {{
        {encoded_cases}  
    }}
    return xdr.ScVal.scvVec(res);
}}

function {name}FromXdr(base64Xdr: string): {name} {{
    type Tag = {name}["tag"];
    type Value = {name}["values"];
    let [tag, values] = strToScVal(base64Xdr).vec()!.map(scValToJs) as [Tag, Value];
    if (!tag) {{
        throw new Error('Missing enum tag when decoding {name} from XDR');
    }}
    return {{ tag, values }} as {name};
}}
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

function {name}FromXdr(base64Xdr: string): {name} {{
    return  scValStrToJs(base64Xdr) as {name};
}}


function {name}ToXdr(val: {name}): xdr.ScVal {{
    return  xdr.ScVal.scvI32(val);
}}
"#,
            )
        }
        Entry::ErrorEnum { doc, cases, .. } => {
            let doc = doc_to_ts_doc(doc);
            let cases = cases
                .iter()
                .map(|c| format!("{{message:\"{}\"}}", c.doc))
                .join(",\n  ");
            format!(
                r#"{doc}const Errors = [ 
{cases}
]"#
            )
        }
    }
}

fn js_to_xdr_fields(struct_name: &str, f: &[StructField]) -> String {
    f.iter()
        .map(|StructField {  name, value , .. }| {
            format!(
                r#"new xdr.ScMapEntry({{key: ((i)=>{})("{name}"), val: ((i)=>{})({struct_name}["{name}"])}})"#,
                type_to_js_xdr(&Type::Symbol),
                type_to_js_xdr(value),
            )
        })
        .join(",\n        ")
}

fn js_to_xdr_union_cases(arg_name: &str, f: &[UnionCase]) -> String {
    f.iter()
        .map(|UnionCase { name, values, .. }| {
            let mut rhs = format!(
                "res.push(((i) => {})(\"{name}\"))",
                type_to_js_xdr(&Type::Symbol)
            );
            if !values.is_empty() {
                for (i, value) in values.iter().enumerate() {
                    rhs = format!(
                        "{rhs};\n            res.push(((i)=>{})({arg_name}.values[{i}]))",
                        type_to_js_xdr(value)
                    );
                }
            };
            format!("case \"{name}\":\n            {rhs};\n            break;")
        })
        .join("\n    ")
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
                format!("[{}]", elements.iter().map(type_to_ts).join(", "))
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

fn js_from_xdr_fields(f: &[StructField]) -> String {
    f.iter()
        .map(|StructField { name, value, .. }| {
            format!(
                r#"{name}: scValToJs(map.get("{name}")) as unknown as {}"#,
                type_to_ts(value)
            )
        })
        .join(",\n        ")
}
