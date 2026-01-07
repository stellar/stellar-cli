#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]

use std::{fs, io};

use crate::types::Type;
use itertools::Itertools;
use sha2::{Digest, Sha256};
use stellar_xdr::curr::{Limits, ScSpecEntry, WriteXdr};

use types::{Entry, ErrorEnumCase};

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
    Parse(stellar_xdr::curr::Error),
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

fn generate_class(
    fns: &[Entry],
    constructor_args: Option<Vec<types::FunctionInput>>,
    spec: &[ScSpecEntry],
) -> String {
    let (constructor_args_in, constructor_args_out) = if let Some(inputs) = constructor_args {
        let Some((args, arg_types)) = args_to_ts(&inputs) else {
            panic!("inputs is present but couldn't be parsed by args_to_ts()");
        };
        (
            format!(
                "
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {args}: {arg_types},",
            ),
            args,
        )
    } else {
        (String::new(), "null".to_string())
    };
    let method_types = fns.iter().map(entry_to_method_type).join("");
    let from_jsons = fns
        .iter()
        .filter_map(entry_to_name_and_return_type)
        .map(|(method, return_type)| format!("{method}: this.txFromJSON<{return_type}>"))
        .join(",\n        ");
    let spec = spec
        .iter()
        .map(|s| format!("\"{}\"", s.to_xdr_base64(Limits::none()).unwrap()))
        .join(",\n        ");
    format!(
        r#"export interface Client {{{method_types}
}}
export class Client extends ContractClient {{
  static async deploy<T = Client>({constructor_args_in}
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {{
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {{@link Operation.createCustomContract}}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }}
  ): Promise<AssembledTransaction<T>> {{
    return ContractClient.deploy({constructor_args_out}, options)
  }}
  constructor(public readonly options: ContractClientOptions) {{
    super(
      new ContractSpec([ {spec} ]),
      options
    )
  }}
  public readonly fromJSON = {{
    {from_jsons}
  }}
}}"#,
    )
}

pub fn generate(spec: &[ScSpecEntry]) -> String {
    let collected: Vec<_> = spec.iter().map(Entry::from).collect();
    let mut constructor_args: Option<Vec<types::FunctionInput>> = None;
    // Filter out function entries with names that start with "__" and partition the results
    for entry in &collected {
        match entry {
            Entry::Function { name, inputs, .. } if name == "__constructor" => {
                if !inputs.is_empty() {
                    constructor_args = Some(inputs.clone());
                }
            }
            _ => {}
        }
    }
    let (fns, other): (Vec<_>, Vec<_>) = collected
        .into_iter()
        .filter(|entry| !matches!(entry, Entry::Function { name, .. } if name.starts_with("__")))
        .partition(|entry| matches!(entry, Entry::Function { .. }));
    let top = other.iter().map(entry_to_method_type).join("\n");
    let bottom = generate_class(&fns, constructor_args, spec);
    format!("{top}\n{bottom}")
}

fn doc_to_ts_doc(doc: &str, method: Option<&str>, indent_level: usize) -> String {
    let indent = "  ".repeat(indent_level);
    let safe_doc = sanitize_doc(doc);

    if let Some(method) = method {
        let safe_doc = if safe_doc.is_empty() {
            String::new()
        } else {
            format!(
                "\n{}   * {}",
                indent,
                safe_doc.split('\n').join(&format!("\n{indent}   * "))
            )
        };
        let safe_method = sanitize_identifier(method);
        return format!(
            r"{indent}/**
{indent}   * Construct and simulate a {safe_method} transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.{safe_doc}
{indent}   */"
        );
    }

    if safe_doc.is_empty() {
        return String::new();
    }

    let safe_doc = safe_doc.split('\n').join(&format!("\n{indent} * "));
    format!(
        r"{indent}/**
{indent} * {safe_doc}
{indent} */
"
    )
}

pub fn entry_to_name_and_return_type(entry: &Entry) -> Option<(String, String)> {
    if let Entry::Function { name, outputs, .. } = entry {
        Some((sanitize_identifier(name), outputs_to_return_type(outputs)))
    } else {
        None
    }
}

pub fn outputs_to_return_type(outputs: &[Type]) -> String {
    match outputs {
        [] => "null".to_owned(),
        [output] => type_to_ts(output),
        outputs => format!("readonly [{}]", outputs.iter().map(type_to_ts).join(", ")),
    }
}

/// Convert a function's inputs to TypeScript arguments. Returns a tuple with the arguments
/// as they'll actually be used in JS, as well as TS type definitions for the arguments.
pub fn args_to_ts(inputs: &[types::FunctionInput]) -> Option<(String, String)> {
    if inputs.is_empty() {
        None
    } else {
        let input_vals = inputs.iter().map(func_input_to_arg_name).join(", ");
        let input_types = inputs.iter().map(func_input_to_ts).join(", ");
        Some((format!("{{{input_vals}}}"), format!("{{{input_types}}}")))
    }
}

#[allow(clippy::too_many_lines)]
pub fn entry_to_method_type(entry: &Entry) -> String {
    match entry {
        Entry::Function {
            doc,
            name,
            inputs,
            outputs,
            ..
        } => {
            let input = if let Some((args, arg_types)) = args_to_ts(inputs) {
                format!("{args}: {arg_types}, ")
            } else {
                String::new()
            };
            let doc = doc_to_ts_doc(doc, Some(name), 0);
            let safe_name = sanitize_identifier(name);
            let return_type = outputs_to_return_type(outputs);
            format!(
                r"
  {doc}
  {safe_name}: ({input}options?: MethodOptions) => Promise<AssembledTransaction<{return_type}>>
"
            )
        }

        Entry::Struct { doc, name, fields } => {
            let docs = doc_to_ts_doc(doc, None, 0);
            let safe_name = sanitize_identifier(name);
            let fields = fields.iter().map(field_to_ts).join("\n  ");
            format!(
                r"
{docs}export interface {safe_name} {{
  {fields}
}}
"
            )
        }

        Entry::TupleStruct { doc, name, fields } => {
            let docs = doc_to_ts_doc(doc, None, 0);
            let safe_name = sanitize_identifier(name);
            let fields = fields.iter().map(type_to_ts).join(",  ");
            format!("{docs}export type {safe_name} = readonly [{fields}];\n")
        }
        Entry::Union { name, doc, cases } => {
            let doc = doc_to_ts_doc(doc, None, 0);
            let safe_name = sanitize_identifier(name);
            let cases = cases.iter().map(case_to_ts).join(" | ");

            format!(
                r"{doc}export type {safe_name} = {cases};
"
            )
        }
        Entry::Enum { doc, name, cases } => {
            let doc = doc_to_ts_doc(doc, None, 0);
            let safe_name = sanitize_identifier(name);
            let cases = cases.iter().map(enum_case_to_ts).join("\n  ");
            let safe_name = if safe_name == "Error" {
                format!("{safe_name}s")
            } else {
                safe_name.clone()
            };
            format!(
                r"{doc}export enum {safe_name} {{
  {cases}
}}
",
            )
        }
        Entry::ErrorEnum { doc, cases, name } => {
            let doc = doc_to_ts_doc(doc, None, 0);
            let safe_name = sanitize_identifier(name);
            let cases = cases.iter().map(error_case_to_ts).join(",\n");
            let safe_name = if safe_name == "Error" {
                format!("{safe_name}s")
            } else {
                safe_name.clone()
            };
            format!(
                r"{doc}export const {safe_name} = {{
{cases}
}}
",
            )
        }
        Entry::Event { doc: _, name: _ } => String::new(),
    }
}

fn error_case_to_ts(ErrorEnumCase { doc, value, name }: &types::ErrorEnumCase) -> String {
    let doc = doc_to_ts_doc(doc, None, 1);
    let name = sanitize_string(name);
    format!("{doc}  {value}: {{message:\"{name}\"}}")
}

fn enum_case_to_ts(case: &types::EnumCase) -> String {
    let types::EnumCase { name, value, .. } = case;
    let name = sanitize_identifier(name);
    format!("{name} = {value},")
}

fn case_to_ts(case: &types::UnionCase) -> String {
    let types::UnionCase { name, values, .. } = case;
    let name = sanitize_string(name);
    format!(
        "{{tag: \"{name}\", values: {}}}",
        type_to_ts(&Type::Tuple {
            elements: values.clone(),
        })
    )
}

fn field_to_ts(field: &types::StructField) -> String {
    let types::StructField { doc, name, value } = field;
    let safe_name = sanitize_identifier(name);
    let doc = doc_to_ts_doc(doc, None, 0);
    let type_ = type_to_ts(value);
    format!("{doc}{safe_name}: {type_};")
}

pub fn func_input_to_ts(input: &types::FunctionInput) -> String {
    let types::FunctionInput { name, value, .. } = input;
    let safe_name = sanitize_identifier(name);
    let type_ = type_to_ts(value);
    format!("{safe_name}: {type_}")
}

pub fn func_input_to_arg_name(input: &types::FunctionInput) -> String {
    let types::FunctionInput { name, .. } = input;
    sanitize_identifier(name)
}

pub fn parse_arg_to_scval(input: &types::FunctionInput) -> String {
    let types::FunctionInput { name, value, .. } = input;
    let safe_name = sanitize_identifier(name);
    match value {
        types::Type::Address => format!("{safe_name}: new Address({safe_name})"),
        _ => safe_name.clone(),
    }
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
        types::Type::Vec { element } => format!("Array<{}>", type_to_ts(element)),
        types::Type::Tuple { elements } => {
            if elements.is_empty() {
                "void".to_owned()
            } else {
                format!("readonly [{}]", elements.iter().map(type_to_ts).join(", "))
            }
        }
        types::Type::Custom { name } => sanitize_identifier(name),
        // TODO: Figure out what js type to map this to. There is already an `Error_` one that
        // ahalabs have added in the bindings, so.. maybe rename that?
        types::Type::Val => "any".to_owned(),
        types::Type::Error { .. } => "Error_".to_owned(),
        types::Type::Address | types::Type::MuxedAddress => "string".to_string(),
        types::Type::Bytes | types::Type::BytesN { .. } => "Buffer".to_string(),
        types::Type::Void => "void".to_owned(),
        types::Type::U256 => "u256".to_string(),
        types::Type::I256 => "i256".to_string(),
        types::Type::Timepoint => "Timepoint".to_string(),
        types::Type::Duration => "Duration".to_string(),
    }
}

/// Sanitize a docstring to be safely included in a TypeScript comment block.
fn sanitize_doc(doc: &str) -> String {
    doc.replace("*/", "* /")
}

/// Sanitize a string to be a valid TypeScript identifier. This only replaces invalid
/// characters with underscores. Valid characters are letters (a-z, A-Z),
/// digits (0-9), underscores (_), and dollar signs ($).
///
/// This does **not** guarantee that the result is a syntactically valid TypeScript identifier.
fn sanitize_identifier(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '$' => c,
            _ => '_',
        })
        .collect::<String>()
}

/// Escape a string for use in a TypeScript string literal
fn sanitize_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Entry, EnumCase, ErrorEnumCase, FunctionInput, StructField, UnionCase};

    const DOC_TEST: &str = "*/ fn()";
    const METHOD_TEST: &str = "; fn() //";
    const STRING_TEST: &str = "\"; fn(); \"";

    #[test]
    fn test_sanitize_doc() {
        assert_eq!(sanitize_doc("hello */ world /*"), "hello * / world /*");
        assert_eq!(sanitize_doc("*/*/"), "* /* /");
        assert_eq!(sanitize_doc("normal text"), "normal text");
        assert_eq!(sanitize_doc(""), "");
    }

    #[test]
    fn test_sanitize_identifier() {
        assert_eq!(sanitize_identifier("hello-world"), "hello_world");
        assert_eq!(sanitize_identifier("test.field"), "test_field");
        assert_eq!(sanitize_identifier("my/path"), "my_path");
        assert_eq!(sanitize_identifier("space test"), "space_test");
        assert_eq!(sanitize_identifier("hello@world"), "hello_world");
        assert_eq!(sanitize_identifier("hello世界"), "hello__");
        assert_eq!(sanitize_identifier("🚀rocket"), "_rocket");
        assert_eq!(sanitize_identifier("$jquery_Name123"), "$jquery_Name123");
        assert_eq!(sanitize_identifier(""), "");
    }

    #[test]
    fn test_sanitize_string() {
        assert_eq!(sanitize_string("hello\"world"), "hello\\\"world");
        assert_eq!(sanitize_string("path\\to\\file"), "path\\\\to\\\\file");
        assert_eq!(sanitize_string("line1\nline2"), "line1\\nline2");
        assert_eq!(sanitize_string("\"; fn(); \""), "\\\"; fn(); \\\"");
        assert_eq!(
            sanitize_string("This is a teapot 123!"),
            "This is a teapot 123!"
        );
        assert_eq!(sanitize_string(""), "");
    }

    #[test]
    fn test_doc_to_ts_doc_no_method_sanitizes() {
        let result = doc_to_ts_doc(DOC_TEST, None, 0);

        assert!(!result.contains(DOC_TEST));
    }

    #[test]
    fn test_doc_to_ts_doc_method_sanitizes() {
        let result = doc_to_ts_doc(DOC_TEST, Some(METHOD_TEST), 0);

        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_entry_to_method_type_function_sanitizes() {
        let entry = Entry::Function {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            inputs: vec![],
            outputs: vec![],
        };

        let result = entry_to_method_type(&entry);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_entry_to_method_type_struct_sanitizes() {
        let entry = Entry::Struct {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            fields: vec![],
        };

        let result = entry_to_method_type(&entry);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_entry_to_method_type_tuple_struct_sanitizes() {
        let entry = Entry::TupleStruct {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            fields: vec![],
        };

        let result = entry_to_method_type(&entry);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_entry_to_method_type_union_sanitizes() {
        let entry = Entry::Union {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            cases: vec![],
        };

        let result = entry_to_method_type(&entry);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_entry_to_method_type_enum_sanitizes() {
        let entry = Entry::Enum {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            cases: vec![],
        };

        let result = entry_to_method_type(&entry);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_entry_to_method_type_error_enum_sanitizes() {
        let entry = Entry::ErrorEnum {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            cases: vec![],
        };

        let result = entry_to_method_type(&entry);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_field_to_ts_sanitizes() {
        let field = StructField {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            value: Type::String,
        };

        let result = field_to_ts(&field);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_func_input_to_ts_sanitizes() {
        let input = FunctionInput {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            value: Type::String,
        };

        let result = func_input_to_ts(&input);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_func_input_to_arg_name_sanitizes() {
        let input = FunctionInput {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            value: Type::String,
        };
        let result = func_input_to_arg_name(&input);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_error_case_to_ts_sanitizes() {
        let error_case = ErrorEnumCase {
            doc: String::from(DOC_TEST),
            value: 1,
            name: String::from(STRING_TEST),
        };

        let result = error_case_to_ts(&error_case);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(STRING_TEST));
    }

    #[test]
    fn test_enum_case_to_ts_sanitizes() {
        let enum_case = EnumCase {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            value: 1,
        };

        let result = enum_case_to_ts(&enum_case);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_case_to_ts_sanitizes() {
        let union_case = UnionCase {
            doc: String::from(DOC_TEST),
            name: String::from(STRING_TEST),
            values: vec![Type::String],
        };

        let result = case_to_ts(&union_case);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(STRING_TEST));
    }

    #[test]
    fn test_type_to_ts_custom_sanitizes() {
        let custom_type = Type::Custom {
            name: String::from(METHOD_TEST),
        };

        let result = type_to_ts(&custom_type);
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_parse_arg_to_scval_address_sanitizes() {
        let input = FunctionInput {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            value: Type::Address,
        };

        let result = parse_arg_to_scval(&input);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }

    #[test]
    fn test_parse_arg_to_scval_string_sanitizes() {
        let input = FunctionInput {
            doc: String::from(DOC_TEST),
            name: String::from(METHOD_TEST),
            value: Type::String,
        };

        let result = parse_arg_to_scval(&input);
        assert!(!result.contains(DOC_TEST));
        assert!(!result.contains(METHOD_TEST));
    }
}
