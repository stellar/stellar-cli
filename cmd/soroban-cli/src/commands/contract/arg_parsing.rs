use crate::commands::contract::arg_parsing::Error::HelpMessage;
use crate::commands::contract::deploy::wasm::CONSTRUCTOR_FUNCTION_NAME;
use crate::commands::txn_result::TxnResult;
use crate::config::{self, sc_address, UnresolvedScAddress};
use crate::print::Print;
use crate::signer::{self, Signer};
use crate::xdr::{
    self, Hash, InvokeContractArgs, ScSpecEntry, ScSpecFunctionV0, ScSpecTypeDef, ScVal, ScVec,
};
use clap::error::ErrorKind::DisplayHelp;
use clap::value_parser;
use heck::ToKebabCase;
use soroban_spec_tools::{sanitize, Spec};
use std::collections::HashMap;
use std::convert::TryInto;
use std::env;
use std::ffi::OsString;
use std::fmt::Debug;
use std::path::PathBuf;
use stellar_xdr::curr::ContractId;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to parse argument '{arg}': {error}\n\nContext: Expected type {expected_type}, but received: '{received_value}'\n\nSuggestion: {suggestion}")]
    CannotParseArg {
        arg: String,
        error: soroban_spec_tools::Error,
        expected_type: String,
        received_value: String,
        suggestion: String,
    },
    #[error("Invalid JSON in argument '{arg}': {json_error}\n\nReceived value: '{received_value}'\n\nSuggestions:\n- Check for missing quotes around strings\n- Ensure proper JSON syntax (commas, brackets, etc.)\n- For complex objects, consider using --{arg}-file-path to load from a file")]
    InvalidJsonArg {
        arg: String,
        json_error: String,
        received_value: String,
    },
    #[error("Type mismatch for argument '{arg}': expected {expected_type}, but got {actual_type}\n\nReceived value: '{received_value}'\n\nSuggestions:\n- For {expected_type}, ensure the value is properly formatted\n- Check the contract specification for the correct argument type")]
    TypeMismatch {
        arg: String,
        expected_type: String,
        actual_type: String,
        received_value: String,
    },
    #[error("Missing required argument '{arg}' of type {expected_type}\n\nSuggestions:\n- Add the argument: --{arg} <value>\n- Or use a file: --{arg}-file-path <path-to-json-file>\n- Check the contract specification for required arguments")]
    MissingArgument { arg: String, expected_type: String },
    #[error("Cannot read file {file_path:?}: {error}\n\nSuggestions:\n- Check if the file exists and is readable\n- Ensure the file path is correct\n- Verify file permissions")]
    MissingFileArg { file_path: PathBuf, error: String },
    #[error("cannot print result {result:?}: {error}")]
    CannotPrintResult {
        result: ScVal,
        error: soroban_spec_tools::Error,
    },
    #[error("function '{function_name}' was not found in the contract\n\nAvailable functions: {available_functions}\n\nSuggestions:\n- Check the function name spelling\n- Use 'stellar contract invoke --help' to see available functions\n- Verify the contract ID is correct")]
    FunctionNotFoundInContractSpec {
        function_name: String,
        available_functions: String,
    },
    #[error("function name '{function_name}' is too long (max 32 characters)\n\nReceived: {function_name} ({length} characters)")]
    FunctionNameTooLong {
        function_name: String,
        length: usize,
    },
    #[error("argument count ({current}) surpasses maximum allowed count ({maximum})\n\nSuggestions:\n- Reduce the number of arguments\n- Consider using file-based arguments for complex data\n- Check if some arguments can be combined")]
    MaxNumberOfArgumentsReached { current: usize, maximum: usize },
    #[error("Unsupported address type '{address}'\n\nSupported formats:\n- Account addresses: G... (starts with G)\n- Contract addresses: C... (starts with C)\n- Muxed accounts: M... (starts with M)\n- Identity names: alice, bob, etc.\n\nReceived: '{address}'")]
    UnsupportedScAddress { address: String },
    #[error("Duplicate map key '{key}' after alias resolution\n\nMultiple input keys resolved to the same address — likely an alias passed alongside its strkey, or two aliases pointing to the same identity.")]
    DuplicateMapKey { key: String },
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    StrVal(#[from] soroban_spec_tools::Error),
    #[error(transparent)]
    ScAddress(#[from] sc_address::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("")]
    HelpMessage(String),
    #[error(transparent)]
    Signer(#[from] signer::Error),
}

pub type HostFunctionParameters = (String, Spec, InvokeContractArgs, Vec<Signer>);

fn running_cmd() -> String {
    let mut args: Vec<String> = env::args().collect();

    if let Some(pos) = args.iter().position(|arg| arg == "--") {
        args.truncate(pos);
    }

    format!("{} --", args.join(" "))
}

pub fn build_host_function_parameters(
    contract_id: &stellar_strkey::Contract,
    slop: &[OsString],
    spec_entries: &[ScSpecEntry],
    config: &config::Args,
) -> Result<HostFunctionParameters, Error> {
    build_host_function_parameters_with_filter(contract_id, slop, spec_entries, config, true)
}

pub fn build_constructor_parameters(
    contract_id: &stellar_strkey::Contract,
    slop: &[OsString],
    spec_entries: &[ScSpecEntry],
    config: &config::Args,
) -> Result<HostFunctionParameters, Error> {
    build_host_function_parameters_with_filter(contract_id, slop, spec_entries, config, false)
}

fn build_host_function_parameters_with_filter(
    contract_id: &stellar_strkey::Contract,
    slop: &[OsString],
    spec_entries: &[ScSpecEntry],
    config: &config::Args,
    filter_constructor: bool,
) -> Result<HostFunctionParameters, Error> {
    let spec = Spec(Some(spec_entries.to_vec()));
    let cmd = build_clap_command(&spec, filter_constructor)?;
    let (function, matches_) = parse_command_matches(cmd, slop)?;
    let func = get_function_spec(&spec, &function)?;
    let (parsed_args, signers) = parse_function_arguments(&func, &matches_, &spec, config)?;
    let invoke_args = build_invoke_contract_args(contract_id, &function, parsed_args)?;

    Ok((function, spec, invoke_args, signers))
}

fn build_clap_command(spec: &Spec, filter_constructor: bool) -> Result<clap::Command, Error> {
    let mut cmd = clap::Command::new(running_cmd())
        .no_binary_name(true)
        .term_width(300)
        .max_term_width(300);

    for ScSpecFunctionV0 { name, .. } in spec.find_functions()? {
        let function_name = name.to_utf8_string_lossy();
        // Filter out the constructor function from the invoke command
        if !filter_constructor || function_name != CONSTRUCTOR_FUNCTION_NAME {
            cmd = cmd.subcommand(build_custom_cmd(&function_name, spec)?);
        }
    }
    cmd.build();
    Ok(cmd)
}

fn parse_command_matches(
    mut cmd: clap::Command,
    slop: &[OsString],
) -> Result<(String, clap::ArgMatches), Error> {
    let long_help = cmd.render_long_help();
    let maybe_matches = cmd.try_get_matches_from(slop);

    let Some((function, matches_)) = (match maybe_matches {
        Ok(mut matches) => matches.remove_subcommand(),
        Err(e) => {
            if e.kind() == DisplayHelp {
                return Err(HelpMessage(e.to_string()));
            }
            e.exit();
        }
    }) else {
        return Err(HelpMessage(format!("{long_help}")));
    };

    Ok((function.clone(), matches_))
}

fn get_function_spec(spec: &Spec, function: &str) -> Result<ScSpecFunctionV0, Error> {
    // Exact match (normal path).
    if let Ok(f) = spec.find_function(function) {
        return Ok(f.clone());
    }
    // Fallback: match against sanitized names for functions whose names contain
    // control characters (clap registers the sanitized form as the command name).
    if let Ok(functions) = spec.find_functions() {
        for f in functions {
            if sanitize(&f.name.to_utf8_string_lossy()) == function {
                return Ok(f.clone());
            }
        }
    }
    Err(Error::FunctionNotFoundInContractSpec {
        function_name: function.to_string(),
        available_functions: get_available_functions(spec),
    })
}

fn parse_function_arguments(
    func: &ScSpecFunctionV0,
    matches_: &clap::ArgMatches,
    spec: &Spec,
    config: &config::Args,
) -> Result<(Vec<ScVal>, Vec<Signer>), Error> {
    let mut parsed_args = Vec::with_capacity(func.inputs.len());
    let mut signers = Vec::<Signer>::new();

    for i in func.inputs.iter() {
        parse_single_argument(i, matches_, spec, config, &mut signers, &mut parsed_args)?;
    }

    Ok((parsed_args, signers))
}

fn parse_single_argument(
    input: &stellar_xdr::curr::ScSpecFunctionInputV0,
    matches_: &clap::ArgMatches,
    spec: &Spec,
    config: &config::Args,
    signers: &mut Vec<Signer>,
    parsed_args: &mut Vec<ScVal>,
) -> Result<(), Error> {
    let name = sanitize(&input.name.to_utf8_string_lossy());
    let expected_type_name = get_type_name(&input.type_); //-0--

    if let Some(mut val) = matches_.get_raw(&name) {
        let s = match val.next() {
            Some(v) => v.to_string_lossy().to_string(),
            None => {
                return Err(Error::MissingArgument {
                    arg: name.clone(),
                    expected_type: expected_type_name,
                });
            }
        };

        // Collect a signer up front for top-level address args, so the
        // alias-named identity can also sign the transaction. Alias-to-strkey
        // resolution itself happens inside parse_argument_with_validation,
        // which uniformly handles top-level and nested Address positions.
        if matches!(
            input.type_,
            ScSpecTypeDef::Address | ScSpecTypeDef::MuxedAddress
        ) {
            let trimmed_s = s.trim_matches('"');
            if let Some(signer) = resolve_signer(trimmed_s, config) {
                signers.push(signer);
            }
        }

        parsed_args.push(parse_argument_with_validation(
            &name,
            &s,
            &input.type_,
            spec,
            config,
        )?);
        Ok(())
    } else if matches!(input.type_, ScSpecTypeDef::Option(_)) {
        parsed_args.push(ScVal::Void);
        Ok(())
    } else if let Some(arg_path) = matches_.get_one::<PathBuf>(&fmt_arg_file_name(&name)) {
        parsed_args.push(parse_file_argument(
            &name,
            arg_path,
            &input.type_,
            expected_type_name,
            spec,
            config,
        )?);
        Ok(())
    } else {
        Err(Error::MissingArgument {
            arg: name,
            expected_type: expected_type_name,
        })
    }
}

fn parse_file_argument(
    name: &str,
    arg_path: &PathBuf,
    type_def: &ScSpecTypeDef,
    expected_type_name: String,
    spec: &Spec,
    config: &config::Args,
) -> Result<ScVal, Error> {
    if matches!(type_def, ScSpecTypeDef::Bytes | ScSpecTypeDef::BytesN(_)) {
        let bytes = std::fs::read(arg_path).map_err(|e| Error::MissingFileArg {
            file_path: arg_path.clone(),
            error: e.to_string(),
        })?;
        ScVal::try_from(&bytes).map_err(|()| Error::CannotParseArg {
            arg: name.to_string(),
            error: soroban_spec_tools::Error::Unknown,
            expected_type: expected_type_name,
            received_value: format!("{} bytes from file", bytes.len()),
            suggestion: "Ensure the file contains valid binary data for the expected byte type"
                .to_string(),
        })
    } else {
        let file_contents =
            std::fs::read_to_string(arg_path).map_err(|e| Error::MissingFileArg {
                file_path: arg_path.clone(),
                error: e.to_string(),
            })?;
        tracing::debug!(
            "file {arg_path:?}, has contents:\n{file_contents}\nAnd type {:#?}\n{}",
            type_def,
            file_contents.len()
        );
        parse_argument_with_validation(name, &file_contents, type_def, spec, config)
    }
}

fn build_invoke_contract_args(
    contract_id: &stellar_strkey::Contract,
    function: &str,
    parsed_args: Vec<ScVal>,
) -> Result<InvokeContractArgs, Error> {
    let contract_address_arg = xdr::ScAddress::Contract(ContractId(Hash(contract_id.0)));
    let function_symbol_arg = function
        .try_into()
        .map_err(|()| Error::FunctionNameTooLong {
            function_name: function.to_string(),
            length: function.len(),
        })?;

    let final_args =
        parsed_args
            .clone()
            .try_into()
            .map_err(|_| Error::MaxNumberOfArgumentsReached {
                current: parsed_args.len(),
                maximum: ScVec::default().max_len(),
            })?;

    Ok(InvokeContractArgs {
        contract_address: contract_address_arg,
        function_name: function_symbol_arg,
        args: final_args,
    })
}

pub fn build_custom_cmd(name: &str, spec: &Spec) -> Result<clap::Command, Error> {
    let func = spec
        .find_function(name)
        .map_err(|_| Error::FunctionNotFoundInContractSpec {
            function_name: name.to_string(),
            available_functions: get_available_functions(spec),
        })?;

    // Parse the function arguments
    let inputs_map = &func
        .inputs
        .iter()
        .map(|i| (sanitize(&i.name.to_utf8_string_lossy()), i.type_.clone()))
        .collect::<HashMap<String, ScSpecTypeDef>>();
    let name: &'static str = Box::leak(sanitize(name).into_boxed_str());
    let mut cmd = clap::Command::new(name)
        .no_binary_name(true)
        .term_width(300)
        .max_term_width(300);
    let kebab_name = name.to_kebab_case();
    if kebab_name != name {
        cmd = cmd.alias(kebab_name);
    }
    let doc: &'static str = Box::leak(sanitize(&func.doc.to_utf8_string_lossy()).into_boxed_str());
    let long_doc: &'static str = Box::leak(arg_file_help(doc).into_boxed_str());

    cmd = cmd.about(Some(doc)).long_about(long_doc);
    for (name, type_) in inputs_map {
        let mut arg = clap::Arg::new(name);
        let file_arg_name = fmt_arg_file_name(name);
        let mut file_arg = clap::Arg::new(&file_arg_name);
        arg = arg
            .long(name)
            .alias(name.to_kebab_case())
            .num_args(1)
            .value_parser(clap::builder::NonEmptyStringValueParser::new())
            .long_help(
                spec.doc(name, type_)?
                    .map(|d| -> &'static str { Box::leak(sanitize(d).into_boxed_str()) }),
            );

        file_arg = file_arg
            .long(&file_arg_name)
            .alias(file_arg_name.to_kebab_case())
            .num_args(1)
            .hide(true)
            .value_parser(value_parser!(PathBuf))
            .conflicts_with(name);

        if let Some(value_name) = spec.arg_value_name(type_, 0) {
            let value_name: &'static str = Box::leak(value_name.into_boxed_str());
            arg = arg.value_name(value_name);
        }

        // Set up special-case arg rules
        arg = match type_ {
            ScSpecTypeDef::Bool => arg
                .num_args(0..1)
                .default_missing_value("true")
                .default_value("false")
                .num_args(0..=1),
            ScSpecTypeDef::Option(_val) => arg.required(false),
            ScSpecTypeDef::I256 | ScSpecTypeDef::I128 | ScSpecTypeDef::I64 | ScSpecTypeDef::I32 => {
                arg.allow_hyphen_values(true)
            }
            _ => arg,
        };

        cmd = cmd.arg(arg);
        cmd = cmd.arg(file_arg);
    }
    Ok(cmd)
}

fn fmt_arg_file_name(name: &str) -> String {
    format!("{name}-file-path")
}

fn arg_file_help(docs: &str) -> String {
    format!(
        r"{docs}
Usage Notes:
Each arg has a corresponding --<arg_name>-file-path which is a path to a file containing the corresponding JSON argument.
Note: The only types which aren't JSON are Bytes and BytesN, which are raw bytes"
    )
}

pub fn output_to_string(
    spec: &Spec,
    res: &ScVal,
    function: &str,
) -> Result<TxnResult<String>, Error> {
    let mut res_str = String::new();
    if let Some(output) = spec.find_function(function)?.outputs.first() {
        res_str = spec
            .xdr_to_json(res, output)
            .map_err(|e| Error::CannotPrintResult {
                result: res.clone(),
                error: e,
            })?
            .to_string();
    }
    Ok(TxnResult::Res(res_str))
}

fn resolve_address(addr_or_alias: &str, config: &config::Args) -> Result<String, Error> {
    let sc_address: UnresolvedScAddress = addr_or_alias.parse().unwrap();
    let account = match sc_address {
        UnresolvedScAddress::Resolved(addr) => addr.to_string(),
        addr @ UnresolvedScAddress::Alias(_) => {
            let addr = addr.resolve(
                &config.locator,
                &config.get_network()?.network_passphrase,
                config.hd_path(),
            )?;
            match addr {
                xdr::ScAddress::Account(account) => account.to_string(),
                contract @ xdr::ScAddress::Contract(_) => contract.to_string(),
                stellar_xdr::curr::ScAddress::MuxedAccount(account) => account.to_string(),
                stellar_xdr::curr::ScAddress::ClaimableBalance(_)
                | stellar_xdr::curr::ScAddress::LiquidityPool(_) => {
                    return Err(Error::UnsupportedScAddress {
                        address: addr.to_string(),
                    })
                }
            }
        }
    };
    Ok(account)
}

fn resolve_signer(addr_or_alias: &str, config: &config::Args) -> Option<Signer> {
    let secret = config.locator.get_secret_key(addr_or_alias).ok()?;
    let print = Print::new(false);
    let signer = secret.signer(config.hd_path(), print).ok()?;
    Some(signer)
}

/// Validates JSON string and returns a more descriptive error if invalid
fn validate_json_arg(arg_name: &str, value: &str) -> Result<(), Error> {
    // Try to parse as JSON first
    if let Err(json_err) = serde_json::from_str::<serde_json::Value>(value) {
        return Err(Error::InvalidJsonArg {
            arg: arg_name.to_string(),
            json_error: json_err.to_string(),
            received_value: value.to_string(),
        });
    }
    Ok(())
}

/// Gets a human-readable type name for error messages
fn get_type_name(type_def: &ScSpecTypeDef) -> String {
    match type_def {
        ScSpecTypeDef::Val => "any value".to_string(),
        ScSpecTypeDef::U64 => "u64 (unsigned 64-bit integer)".to_string(),
        ScSpecTypeDef::I64 => "i64 (signed 64-bit integer)".to_string(),
        ScSpecTypeDef::U128 => "u128 (unsigned 128-bit integer)".to_string(),
        ScSpecTypeDef::I128 => "i128 (signed 128-bit integer)".to_string(),
        ScSpecTypeDef::U32 => "u32 (unsigned 32-bit integer)".to_string(),
        ScSpecTypeDef::I32 => "i32 (signed 32-bit integer)".to_string(),
        ScSpecTypeDef::U256 => "u256 (unsigned 256-bit integer)".to_string(),
        ScSpecTypeDef::I256 => "i256 (signed 256-bit integer)".to_string(),
        ScSpecTypeDef::Bool => "bool (true/false)".to_string(),
        ScSpecTypeDef::Symbol => "symbol (identifier)".to_string(),
        ScSpecTypeDef::String => "string".to_string(),
        ScSpecTypeDef::Bytes => "bytes (raw binary data)".to_string(),
        ScSpecTypeDef::BytesN(n) => format!("bytes{} (exactly {} bytes)", n.n, n.n),
        ScSpecTypeDef::Address => {
            "address (G... for account, C... for contract, or identity name)".to_string()
        }
        ScSpecTypeDef::MuxedAddress => "muxed address (M... or identity name)".to_string(),
        ScSpecTypeDef::Void => "void (no value)".to_string(),
        ScSpecTypeDef::Error => "error".to_string(),
        ScSpecTypeDef::Timepoint => "timepoint (timestamp)".to_string(),
        ScSpecTypeDef::Duration => "duration (time span)".to_string(),
        ScSpecTypeDef::Option(inner) => format!("optional {}", get_type_name(&inner.value_type)),
        ScSpecTypeDef::Vec(inner) => format!("vector of {}", get_type_name(&inner.element_type)),
        ScSpecTypeDef::Map(map_type) => format!(
            "map from {} to {}",
            get_type_name(&map_type.key_type),
            get_type_name(&map_type.value_type)
        ),
        ScSpecTypeDef::Tuple(tuple_type) => {
            let types: Vec<String> = tuple_type.value_types.iter().map(get_type_name).collect();
            format!("tuple({})", types.join(", "))
        }
        ScSpecTypeDef::Result(_) => "result".to_string(),
        ScSpecTypeDef::Udt(udt) => {
            format!(
                "user-defined type '{}'",
                sanitize(&udt.name.to_utf8_string_lossy())
            )
        }
    }
}

/// Gets available function names for error messages
fn get_available_functions(spec: &Spec) -> String {
    match spec.find_functions() {
        Ok(functions) => functions
            .map(|f| sanitize(&f.name.to_utf8_string_lossy()))
            .collect::<Vec<_>>()
            .join(", "),
        Err(_) => "unknown".to_string(),
    }
}

/// Checks if a type is a primitive type that doesn't require JSON validation
fn is_primitive_type(type_def: &ScSpecTypeDef) -> bool {
    matches!(
        type_def,
        ScSpecTypeDef::U32
            | ScSpecTypeDef::U64
            | ScSpecTypeDef::U128
            | ScSpecTypeDef::U256
            | ScSpecTypeDef::I32
            | ScSpecTypeDef::I64
            | ScSpecTypeDef::I128
            | ScSpecTypeDef::I256
            | ScSpecTypeDef::Bool
            | ScSpecTypeDef::Symbol
            | ScSpecTypeDef::String
            | ScSpecTypeDef::Bytes
            | ScSpecTypeDef::BytesN(_)
            | ScSpecTypeDef::Address
            | ScSpecTypeDef::MuxedAddress
            | ScSpecTypeDef::Timepoint
            | ScSpecTypeDef::Duration
            | ScSpecTypeDef::Void
    )
}

/// Generates context-aware suggestions based on the expected type and error
fn get_context_suggestions(expected_type: &ScSpecTypeDef, received_value: &str) -> String {
    match expected_type {
        ScSpecTypeDef::U64 | ScSpecTypeDef::I64 | ScSpecTypeDef::U128 | ScSpecTypeDef::I128
        | ScSpecTypeDef::U32 | ScSpecTypeDef::I32 | ScSpecTypeDef::U256 | ScSpecTypeDef::I256 => {
            if received_value.starts_with('"') && received_value.ends_with('"') {
                "For numbers, ensure no quotes around the value (e.g., use 100 instead of \"100\")".to_string()
            } else if received_value.contains('.') {
                "Integer types don't support decimal values - use a whole number".to_string()
            } else {
                "Ensure the value is a valid integer within the type's range".to_string()
            }
        }
        ScSpecTypeDef::Bool => {
            "For booleans, use 'true' or 'false' (without quotes)".to_string()
        }
        ScSpecTypeDef::String => {
            if !received_value.starts_with('"') || !received_value.ends_with('"') {
                "For strings, ensure the value is properly quoted (e.g., \"hello world\")".to_string()
            } else {
                "Check for proper string escaping if the string contains special characters".to_string()
            }
        }
        ScSpecTypeDef::Address => {
            "For addresses, use format: G... (account), C... (contract), or identity name (e.g., alice)".to_string()
        }
        ScSpecTypeDef::MuxedAddress => {
            "For muxed addresses, use format: M... or identity name".to_string()
        }
        ScSpecTypeDef::Vec(_) => {
            "For arrays, use JSON array format: [\"item1\", \"item2\"] or [{\"key\": \"value\"}]".to_string()
        }
        ScSpecTypeDef::Map(_) => {
            "For maps, use JSON object format: {\"key1\": \"value1\", \"key2\": \"value2\"}".to_string()
        }
        ScSpecTypeDef::Option(_) => {
            "For optional values, use null for none or the expected value type".to_string()
        }
        _ => {
            "Check the contract specification for the correct argument format and type".to_string()
        }
    }
}

/// Enhanced argument parsing with better error handling
fn parse_argument_with_validation(
    arg_name: &str,
    value: &str,
    expected_type: &ScSpecTypeDef,
    spec: &Spec,
    config: &config::Args,
) -> Result<ScVal, Error> {
    let expected_type_name = get_type_name(expected_type);

    // Pre-validate JSON for non-primitive types, but skip for union (enum) UDTs since
    // both bare strings (e.g. `Unit`) and JSON strings (e.g. `"Unit"`) are valid for
    // unit variants — from_string in soroban-spec-tools handles both forms correctly.
    let is_union_udt = if let ScSpecTypeDef::Udt(udt) = expected_type {
        spec.find(&udt.name.to_utf8_string_lossy())
            .is_ok_and(|entry| matches!(entry, ScSpecEntry::UdtUnionV0(_)))
    } else {
        false
    };
    if !is_primitive_type(expected_type) && !is_union_udt {
        validate_json_arg(arg_name, value)?;
    }

    // Walk the input through resolve_aliases_in_json so identity aliases are
    // resolved at every Address/MuxedAddress position, top-level or nested.
    let resolved = resolve_aliases(value, expected_type, spec, config)?;

    spec.from_string(&resolved, expected_type)
        .map_err(|error| Error::CannotParseArg {
            arg: arg_name.to_string(),
            error,
            expected_type: expected_type_name,
            received_value: value.to_string(),
            suggestion: get_context_suggestions(expected_type, value),
        })
}

/// Returns the input with identity aliases resolved to strkeys at every
/// `Address`/`MuxedAddress` position the spec describes. Inputs that aren't
/// JSON (e.g. a bare top-level alias `alice`) are wrapped as a JSON string
/// for `Address`-typed args so the walker can still resolve them.
fn resolve_aliases(
    value: &str,
    type_def: &ScSpecTypeDef,
    spec: &Spec,
    config: &config::Args,
) -> Result<String, Error> {
    let is_address = matches!(
        type_def,
        ScSpecTypeDef::Address | ScSpecTypeDef::MuxedAddress
    );

    let mut json = match serde_json::from_str::<serde_json::Value>(value) {
        Ok(j) => j,
        Err(_) if is_address => serde_json::Value::String(value.trim_matches('"').to_string()),
        Err(_) => return Ok(value.to_string()),
    };

    let mutated = resolve_aliases_in_json(&mut json, type_def, spec, config)?;

    // Nothing was rewritten — return the original input verbatim so we don't
    // disturb whitespace, key ordering, or number formatting just to reparse it.
    if !mutated {
        return Ok(value.to_string());
    }

    // For top-level Address inputs, hand back the bare strkey rather than a
    // JSON-quoted form — `Spec::from_string` accepts both, but the bare form
    // matches what the original Address path produced.
    Ok(match (&json, is_address) {
        (serde_json::Value::String(s), true) => s.clone(),
        _ => json.to_string(),
    })
}

/// Walks a JSON value alongside the contract spec type tree, rewriting any
/// string at an `Address`/`MuxedAddress` position into the resolved address
/// via the locator. Strings that are already a valid account, contract, or
/// muxed strkey pass through unchanged.
///
/// This makes identity aliases work inside nested arguments (struct fields,
/// vec/map/tuple elements, option values, union tuple-variant payloads), not
/// just at the top level. Returns whether any string was actually rewritten.
fn resolve_aliases_in_json(
    value: &mut serde_json::Value,
    type_def: &ScSpecTypeDef,
    spec: &Spec,
    config: &config::Args,
) -> Result<bool, Error> {
    let mut mutated = false;
    match type_def {
        ScSpecTypeDef::Address | ScSpecTypeDef::MuxedAddress => {
            if let serde_json::Value::String(s) = value {
                let resolved = resolve_address(s, config)?;
                if &resolved != s {
                    *s = resolved;
                    mutated = true;
                }
            }
        }
        ScSpecTypeDef::Vec(inner) => {
            if let serde_json::Value::Array(arr) = value {
                for item in arr.iter_mut() {
                    mutated |= resolve_aliases_in_json(item, &inner.element_type, spec, config)?;
                }
            }
        }
        ScSpecTypeDef::Tuple(tuple) => {
            if let serde_json::Value::Array(arr) = value {
                for (item, ty) in arr.iter_mut().zip(tuple.value_types.iter()) {
                    mutated |= resolve_aliases_in_json(item, ty, spec, config)?;
                }
            }
        }
        ScSpecTypeDef::Map(map) => {
            if let serde_json::Value::Object(obj) = value {
                let key_is_address = matches!(
                    map.key_type.as_ref(),
                    ScSpecTypeDef::Address | ScSpecTypeDef::MuxedAddress
                );
                if key_is_address {
                    let entries = std::mem::take(obj);
                    for (k, mut v) in entries {
                        mutated |= resolve_aliases_in_json(&mut v, &map.value_type, spec, config)?;
                        let resolved = resolve_address(&k, config)?;
                        if resolved != k {
                            mutated = true;
                        }
                        if obj.contains_key(&resolved) {
                            return Err(Error::DuplicateMapKey { key: resolved });
                        }
                        obj.insert(resolved, v);
                    }
                } else {
                    for v in obj.values_mut() {
                        mutated |= resolve_aliases_in_json(v, &map.value_type, spec, config)?;
                    }
                }
            }
        }
        ScSpecTypeDef::Option(inner) if !matches!(value, serde_json::Value::Null) => {
            mutated |= resolve_aliases_in_json(value, &inner.value_type, spec, config)?;
        }
        ScSpecTypeDef::Result(result) => {
            // Result is rarely used as an input type. The walker descends into
            // both branches; the inner `match value` no-ops when the JSON
            // shape doesn't fit the branch's type. Resolution is idempotent
            // (a strkey re-resolves to itself), so descending twice is safe
            // when both branches happen to share a shape.
            mutated |= resolve_aliases_in_json(value, &result.ok_type, spec, config)?;
            mutated |= resolve_aliases_in_json(value, &result.error_type, spec, config)?;
        }
        ScSpecTypeDef::Udt(udt) => {
            mutated |= resolve_aliases_in_udt(value, udt, spec, config)?;
        }
        _ => {}
    }
    Ok(mutated)
}

fn resolve_aliases_in_udt(
    value: &mut serde_json::Value,
    udt: &stellar_xdr::curr::ScSpecTypeUdt,
    spec: &Spec,
    config: &config::Args,
) -> Result<bool, Error> {
    let mut mutated = false;
    let name = udt.name.to_utf8_string_lossy();
    let Ok(entry) = spec.find(&name) else {
        return Ok(false);
    };
    match entry {
        ScSpecEntry::UdtStructV0(strukt) => {
            // Soroban's contract macros emit numeric field names ("0", "1", …)
            // for tuple structs and identifier names for regular structs, so a
            // field literally named "0" reliably distinguishes the two.
            let is_tuple_struct = strukt
                .fields
                .iter()
                .any(|f| f.name.to_utf8_string_lossy() == "0");
            match value {
                serde_json::Value::Array(arr) if is_tuple_struct => {
                    for (item, field) in arr.iter_mut().zip(strukt.fields.iter()) {
                        mutated |= resolve_aliases_in_json(item, &field.type_, spec, config)?;
                    }
                }
                serde_json::Value::Object(obj) => {
                    for field in strukt.fields.iter() {
                        let key = field.name.to_utf8_string_lossy();
                        if let Some(field_val) = obj.get_mut(key.as_str()) {
                            mutated |=
                                resolve_aliases_in_json(field_val, &field.type_, spec, config)?;
                        }
                    }
                }
                _ => {}
            }
        }
        ScSpecEntry::UdtUnionV0(union) => {
            mutated |= resolve_aliases_in_union(value, union, spec, config)?;
        }
        _ => {}
    }
    Ok(mutated)
}

fn resolve_aliases_in_union(
    value: &mut serde_json::Value,
    union: &stellar_xdr::curr::ScSpecUdtUnionV0,
    spec: &Spec,
    config: &config::Args,
) -> Result<bool, Error> {
    use stellar_xdr::curr::ScSpecUdtUnionCaseV0;

    let serde_json::Value::Object(obj) = value else {
        return Ok(false);
    };
    let Some((case_name, payload)) = obj.iter_mut().next() else {
        return Ok(false);
    };
    let matched = union.cases.iter().find_map(|c| match c {
        ScSpecUdtUnionCaseV0::TupleV0(t) if t.name.to_utf8_string_lossy() == *case_name => Some(t),
        _ => None,
    });
    let Some(tuple) = matched else {
        return Ok(false);
    };
    // Single-element tuple variants take a bare payload — `{"Variant": value}` —
    // matching the form `soroban_spec_tools` accepts. Variants with two or more
    // elements take an array payload — `{"Variant": [a, b, ...]}`.
    if tuple.type_.len() == 1 {
        return resolve_aliases_in_json(payload, &tuple.type_[0], spec, config);
    }
    let mut mutated = false;
    if let serde_json::Value::Array(arr) = payload {
        for (item, ty) in arr.iter_mut().zip(tuple.type_.iter()) {
            mutated |= resolve_aliases_in_json(item, ty, spec, config)?;
        }
    }
    Ok(mutated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::curr::{ScSpecTypeBytesN, ScSpecTypeDef, ScSpecTypeOption, ScSpecTypeVec};

    #[test]
    fn test_get_type_name_primitives() {
        assert_eq!(
            get_type_name(&ScSpecTypeDef::U32),
            "u32 (unsigned 32-bit integer)"
        );
        assert_eq!(
            get_type_name(&ScSpecTypeDef::I64),
            "i64 (signed 64-bit integer)"
        );
        assert_eq!(get_type_name(&ScSpecTypeDef::Bool), "bool (true/false)");
        assert_eq!(get_type_name(&ScSpecTypeDef::String), "string");
        assert_eq!(
            get_type_name(&ScSpecTypeDef::Address),
            "address (G... for account, C... for contract, or identity name)"
        );
    }

    #[test]
    fn test_get_type_name_complex() {
        let option_type = ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
            value_type: Box::new(ScSpecTypeDef::U32),
        }));
        assert_eq!(
            get_type_name(&option_type),
            "optional u32 (unsigned 32-bit integer)"
        );

        let vec_type = ScSpecTypeDef::Vec(Box::new(ScSpecTypeVec {
            element_type: Box::new(ScSpecTypeDef::String),
        }));
        assert_eq!(get_type_name(&vec_type), "vector of string");
    }

    #[test]
    fn test_is_primitive_type_all_primitives() {
        assert!(is_primitive_type(&ScSpecTypeDef::U32));
        assert!(is_primitive_type(&ScSpecTypeDef::I32));
        assert!(is_primitive_type(&ScSpecTypeDef::U64));
        assert!(is_primitive_type(&ScSpecTypeDef::I64));
        assert!(is_primitive_type(&ScSpecTypeDef::U128));
        assert!(is_primitive_type(&ScSpecTypeDef::I128));
        assert!(is_primitive_type(&ScSpecTypeDef::U256));
        assert!(is_primitive_type(&ScSpecTypeDef::I256));

        assert!(is_primitive_type(&ScSpecTypeDef::Bool));
        assert!(is_primitive_type(&ScSpecTypeDef::Symbol));
        assert!(is_primitive_type(&ScSpecTypeDef::String));
        assert!(is_primitive_type(&ScSpecTypeDef::Void));
        assert!(is_primitive_type(&ScSpecTypeDef::Bytes));
        assert!(is_primitive_type(&ScSpecTypeDef::BytesN(
            ScSpecTypeBytesN { n: 32 }
        )));
        assert!(is_primitive_type(&ScSpecTypeDef::BytesN(
            ScSpecTypeBytesN { n: 64 }
        )));

        assert!(is_primitive_type(&ScSpecTypeDef::Address));
        assert!(is_primitive_type(&ScSpecTypeDef::MuxedAddress));
        assert!(is_primitive_type(&ScSpecTypeDef::Timepoint));
        assert!(is_primitive_type(&ScSpecTypeDef::Duration));

        assert!(!is_primitive_type(&ScSpecTypeDef::Vec(Box::new(
            ScSpecTypeVec {
                element_type: Box::new(ScSpecTypeDef::U32),
            }
        ))));
    }

    #[test]
    fn test_validate_json_arg_valid() {
        // Valid JSON should not return an error
        assert!(validate_json_arg("test_arg", r#"{"key": "value"}"#).is_ok());
        assert!(validate_json_arg("test_arg", "123").is_ok());
        assert!(validate_json_arg("test_arg", r#""string""#).is_ok());
        assert!(validate_json_arg("test_arg", "true").is_ok());
        assert!(validate_json_arg("test_arg", "null").is_ok());
    }

    #[test]
    fn test_validate_json_arg_invalid() {
        // Invalid JSON should return an error
        let result = validate_json_arg("test_arg", r#"{"key": value}"#); // Missing quotes around value
        assert!(result.is_err());

        if let Err(Error::InvalidJsonArg {
            arg,
            json_error,
            received_value,
        }) = result
        {
            assert_eq!(arg, "test_arg");
            assert_eq!(received_value, r#"{"key": value}"#);
            assert!(json_error.contains("expected"));
        } else {
            panic!("Expected InvalidJsonArg error");
        }
    }

    #[test]
    fn test_validate_json_arg_malformed() {
        // Test various malformed JSON cases
        let test_cases = vec![
            r#"{"key": }"#,         // Missing value
            r#"{key: "value"}"#,    // Missing quotes around key
            r#"{"key": "value",}"#, // Trailing comma
            r#"{"key" "value"}"#,   // Missing colon
        ];

        for case in test_cases {
            let result = validate_json_arg("test_arg", case);
            assert!(result.is_err(), "Expected error for case: {case}");
        }
    }

    #[test]
    fn test_context_aware_error_messages() {
        use stellar_xdr::curr::ScSpecTypeDef;

        // Test context-aware suggestions for different types

        // Test u64 with quoted value
        let suggestion = get_context_suggestions(&ScSpecTypeDef::U64, "\"100\"");
        assert!(suggestion.contains("no quotes around the value"));
        assert!(suggestion.contains("use 100 instead of \"100\""));

        // Test u64 with decimal value
        let suggestion = get_context_suggestions(&ScSpecTypeDef::U64, "100.5");
        assert!(suggestion.contains("don't support decimal values"));

        // Test string without quotes
        let suggestion = get_context_suggestions(&ScSpecTypeDef::String, "hello");
        assert!(suggestion.contains("properly quoted"));

        // Test address type
        let suggestion = get_context_suggestions(&ScSpecTypeDef::Address, "invalid_addr");
        assert!(suggestion.contains("G... (account), C... (contract)"));

        // Test boolean type
        let suggestion = get_context_suggestions(&ScSpecTypeDef::Bool, "yes");
        assert!(suggestion.contains("'true' or 'false'"));

        println!("=== Context-Aware Error Message Examples ===");
        println!("U64 with quotes: {suggestion}");

        let decimal_suggestion = get_context_suggestions(&ScSpecTypeDef::U64, "100.5");
        println!("U64 with decimal: {decimal_suggestion}");

        let string_suggestion = get_context_suggestions(&ScSpecTypeDef::String, "hello");
        println!("String without quotes: {string_suggestion}");

        let address_suggestion = get_context_suggestions(&ScSpecTypeDef::Address, "invalid");
        println!("Invalid address: {address_suggestion}");
    }

    #[test]
    fn test_union_udt_bare_string_accepted() {
        use stellar_xdr::curr::{
            ScSpecEntry, ScSpecTypeDef, ScSpecTypeUdt, ScSpecUdtUnionCaseV0,
            ScSpecUdtUnionCaseVoidV0, ScSpecUdtUnionV0, StringM,
        };

        // Build a minimal Spec with a union type: enum MyEnum { Unit }
        let union_name: StringM<60> = "MyEnum".try_into().unwrap();
        let case_name: StringM<60> = "Unit".try_into().unwrap();
        let spec = Spec(Some(vec![ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: union_name.clone(),
            cases: vec![ScSpecUdtUnionCaseV0::VoidV0(ScSpecUdtUnionCaseVoidV0 {
                doc: StringM::default(),
                name: case_name,
            })]
            .try_into()
            .unwrap(),
        })]));

        let expected_type = ScSpecTypeDef::Udt(ScSpecTypeUdt { name: union_name });
        let config = crate::config::Args::default();

        // Bare string (no JSON quoting) should be accepted
        let result =
            parse_argument_with_validation("value", "Unit", &expected_type, &spec, &config);
        assert!(result.is_ok(), "bare 'Unit' should be accepted: {result:?}");

        // JSON-quoted string should also be accepted
        let result =
            parse_argument_with_validation("value", "\"Unit\"", &expected_type, &spec, &config);
        assert!(
            result.is_ok(),
            "JSON-quoted '\"Unit\"' should be accepted: {result:?}"
        );

        // Both forms should produce the same ScVal
        let bare = parse_argument_with_validation("value", "Unit", &expected_type, &spec, &config)
            .unwrap();
        let quoted =
            parse_argument_with_validation("value", "\"Unit\"", &expected_type, &spec, &config)
                .unwrap();
        assert_eq!(
            bare, quoted,
            "bare and quoted forms should produce identical ScVal"
        );
    }

    #[test]
    fn test_union_udt_tuple_variant_still_requires_json() {
        use stellar_xdr::curr::{
            ScSpecEntry, ScSpecTypeDef, ScSpecTypeUdt, ScSpecUdtUnionCaseTupleV0,
            ScSpecUdtUnionCaseV0, ScSpecUdtUnionCaseVoidV0, ScSpecUdtUnionV0, StringM,
        };

        let union_name: StringM<60> = "MyEnum".try_into().unwrap();
        let spec = Spec(Some(vec![ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: union_name.clone(),
            cases: vec![
                ScSpecUdtUnionCaseV0::VoidV0(ScSpecUdtUnionCaseVoidV0 {
                    doc: StringM::default(),
                    name: "Unit".try_into().unwrap(),
                }),
                ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                    doc: StringM::default(),
                    name: "WithValue".try_into().unwrap(),
                    type_: vec![ScSpecTypeDef::U32].try_into().unwrap(),
                }),
            ]
            .try_into()
            .unwrap(),
        })]));

        let expected_type = ScSpecTypeDef::Udt(ScSpecTypeUdt { name: union_name });
        let config = crate::config::Args::default();

        // Tuple variant with a value must still use JSON object syntax
        let result = parse_argument_with_validation(
            "value",
            r#"{"WithValue":42}"#,
            &expected_type,
            &spec,
            &config,
        );
        assert!(
            result.is_ok(),
            "JSON object for tuple variant should be accepted: {result:?}"
        );
    }

    #[test]
    fn test_error_message_format() {
        use stellar_xdr::curr::ScSpecTypeDef;

        // Test that our CannotParseArg error formats correctly
        let error = Error::CannotParseArg {
            arg: "amount".to_string(),
            error: soroban_spec_tools::Error::InvalidValue(Some(ScSpecTypeDef::U64)),
            expected_type: "u64 (unsigned 64-bit integer)".to_string(),
            received_value: "\"100\"".to_string(),
            suggestion:
                "For numbers, ensure no quotes around the value (e.g., use 100 instead of \"100\")"
                    .to_string(),
        };

        let error_message = format!("{error}");
        println!("\n=== Complete Error Message Example ===");
        println!("{error_message}");

        // Verify the error message contains all expected parts
        assert!(error_message.contains("Failed to parse argument 'amount'"));
        assert!(error_message.contains("Expected type u64 (unsigned 64-bit integer)"));
        assert!(error_message.contains("received: '\"100\"'"));
        assert!(error_message.contains("Suggestion: For numbers, ensure no quotes"));
    }

    fn struct_spec(name: &'static str, fields: &[(&str, ScSpecTypeDef)]) -> (Spec, ScSpecTypeDef) {
        use stellar_xdr::curr::{
            ScSpecEntry, ScSpecTypeUdt, ScSpecUdtStructFieldV0, ScSpecUdtStructV0, StringM,
        };
        let struct_name: StringM<60> = name.try_into().unwrap();
        let fields_xdr: Vec<ScSpecUdtStructFieldV0> = fields
            .iter()
            .map(|(n, t)| ScSpecUdtStructFieldV0 {
                doc: StringM::default(),
                name: (*n).try_into().unwrap(),
                type_: t.clone(),
            })
            .collect();
        let spec = Spec(Some(vec![ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: struct_name.clone(),
            fields: fields_xdr.try_into().unwrap(),
        })]));
        let ty = ScSpecTypeDef::Udt(ScSpecTypeUdt { name: struct_name });
        (spec, ty)
    }

    // A real account strkey that should pass through resolve_address unchanged.
    const TEST_G_ADDRESS: &str = "GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS";

    #[test]
    fn resolve_aliases_in_json_walks_vec_of_address() {
        use stellar_xdr::curr::ScSpecTypeVec;

        let ty = ScSpecTypeDef::Vec(Box::new(ScSpecTypeVec {
            element_type: Box::new(ScSpecTypeDef::Address),
        }));
        let spec = Spec(Some(vec![]));
        let config = crate::config::Args::default();

        let mut value = serde_json::json!([TEST_G_ADDRESS]);
        resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap();
        assert_eq!(value, serde_json::json!([TEST_G_ADDRESS]));

        // An unknown alias-shaped string at a nested Address position must surface as an error.
        let mut value = serde_json::json!(["definitely-not-a-known-alias"]);
        let err = resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap_err();
        assert!(
            matches!(err, Error::Config(_) | Error::ScAddress(_)),
            "expected alias-resolution error, got {err:?}"
        );
    }

    #[test]
    fn resolve_aliases_in_json_walks_tuple() {
        use stellar_xdr::curr::ScSpecTypeTuple;

        let ty = ScSpecTypeDef::Tuple(Box::new(ScSpecTypeTuple {
            value_types: vec![ScSpecTypeDef::Address, ScSpecTypeDef::U32]
                .try_into()
                .unwrap(),
        }));
        let spec = Spec(Some(vec![]));
        let config = crate::config::Args::default();

        let mut value = serde_json::json!([TEST_G_ADDRESS, 42]);
        resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap();
        assert_eq!(value, serde_json::json!([TEST_G_ADDRESS, 42]));

        let mut value = serde_json::json!(["bogus-alias", 42]);
        let err = resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap_err();
        assert!(
            matches!(err, Error::Config(_) | Error::ScAddress(_)),
            "expected alias-resolution error, got {err:?}"
        );
    }

    #[test]
    fn resolve_aliases_in_json_walks_struct_field() {
        use stellar_xdr::curr::ScSpecTypeVec;

        let (spec, ty) = struct_spec(
            "Operator",
            &[
                ("count", ScSpecTypeDef::U32),
                (
                    "addresses",
                    ScSpecTypeDef::Vec(Box::new(ScSpecTypeVec {
                        element_type: Box::new(ScSpecTypeDef::Address),
                    })),
                ),
            ],
        );
        let config = crate::config::Args::default();

        let mut value = serde_json::json!({"count": 1, "addresses": [TEST_G_ADDRESS]});
        resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"count": 1, "addresses": [TEST_G_ADDRESS]})
        );

        // Walker must reach the Address inside Vec inside the struct field.
        let mut value = serde_json::json!({"count": 1, "addresses": ["bogus-alias"]});
        let err = resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap_err();
        assert!(
            matches!(err, Error::Config(_) | Error::ScAddress(_)),
            "expected alias-resolution error, got {err:?}"
        );
    }

    #[test]
    fn resolve_aliases_in_json_walks_union_tuple_variant() {
        use stellar_xdr::curr::{
            ScSpecEntry, ScSpecTypeUdt, ScSpecUdtUnionCaseTupleV0, ScSpecUdtUnionCaseV0,
            ScSpecUdtUnionV0, StringM,
        };

        let union_name: StringM<60> = "Choice".try_into().unwrap();
        let spec = Spec(Some(vec![ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: union_name.clone(),
            cases: vec![ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                doc: StringM::default(),
                name: "Pick".try_into().unwrap(),
                type_: vec![ScSpecTypeDef::Address, ScSpecTypeDef::U32]
                    .try_into()
                    .unwrap(),
            })]
            .try_into()
            .unwrap(),
        })]));

        let ty = ScSpecTypeDef::Udt(ScSpecTypeUdt { name: union_name });
        let config = crate::config::Args::default();

        let mut value = serde_json::json!({"Pick": [TEST_G_ADDRESS, 42]});
        resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap();
        assert_eq!(value, serde_json::json!({"Pick": [TEST_G_ADDRESS, 42]}));

        let mut value = serde_json::json!({"Pick": ["bogus-alias", 42]});
        let err = resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap_err();
        assert!(
            matches!(err, Error::Config(_) | Error::ScAddress(_)),
            "expected alias-resolution error, got {err:?}"
        );
    }

    #[test]
    fn resolve_aliases_in_json_walks_single_element_union_variant() {
        use stellar_xdr::curr::{
            ScSpecEntry, ScSpecTypeUdt, ScSpecUdtUnionCaseTupleV0, ScSpecUdtUnionCaseV0,
            ScSpecUdtUnionV0, StringM,
        };

        let union_name: StringM<60> = "OneOf".try_into().unwrap();
        let spec = Spec(Some(vec![ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: union_name.clone(),
            cases: vec![ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                doc: StringM::default(),
                name: "Only".try_into().unwrap(),
                type_: vec![ScSpecTypeDef::Address].try_into().unwrap(),
            })]
            .try_into()
            .unwrap(),
        })]));

        let ty = ScSpecTypeDef::Udt(ScSpecTypeUdt { name: union_name });
        let config = crate::config::Args::default();

        // Bare payload form: {"Only": addr} — not {"Only": [addr]}.
        let mut value = serde_json::json!({"Only": TEST_G_ADDRESS});
        resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap();
        assert_eq!(value, serde_json::json!({"Only": TEST_G_ADDRESS}));

        let mut value = serde_json::json!({"Only": "bogus-alias"});
        let err = resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap_err();
        assert!(
            matches!(err, Error::Config(_) | Error::ScAddress(_)),
            "expected alias-resolution error, got {err:?}"
        );
    }

    #[test]
    fn resolve_aliases_in_json_walks_option_and_map() {
        use stellar_xdr::curr::{ScSpecTypeMap, ScSpecTypeOption};

        let opt_ty = ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
            value_type: Box::new(ScSpecTypeDef::Address),
        }));
        let spec = Spec(Some(vec![]));
        let config = crate::config::Args::default();

        let mut value = serde_json::Value::Null;
        resolve_aliases_in_json(&mut value, &opt_ty, &spec, &config).unwrap();
        assert_eq!(value, serde_json::Value::Null);

        let mut value = serde_json::json!(TEST_G_ADDRESS);
        resolve_aliases_in_json(&mut value, &opt_ty, &spec, &config).unwrap();
        assert_eq!(value, serde_json::json!(TEST_G_ADDRESS));

        let map_ty = ScSpecTypeDef::Map(Box::new(ScSpecTypeMap {
            key_type: Box::new(ScSpecTypeDef::Symbol),
            value_type: Box::new(ScSpecTypeDef::Address),
        }));
        let mut value = serde_json::json!({"owner": TEST_G_ADDRESS});
        resolve_aliases_in_json(&mut value, &map_ty, &spec, &config).unwrap();
        assert_eq!(value, serde_json::json!({"owner": TEST_G_ADDRESS}));

        let mut value = serde_json::json!({"owner": "bogus-alias"});
        let err = resolve_aliases_in_json(&mut value, &map_ty, &spec, &config).unwrap_err();
        assert!(
            matches!(err, Error::Config(_) | Error::ScAddress(_)),
            "expected alias-resolution error, got {err:?}"
        );
    }

    #[test]
    fn resolve_aliases_in_json_walks_result_inner_types() {
        use stellar_xdr::curr::ScSpecTypeResult;

        let ty = ScSpecTypeDef::Result(Box::new(ScSpecTypeResult {
            ok_type: Box::new(ScSpecTypeDef::Address),
            error_type: Box::new(ScSpecTypeDef::U32),
        }));
        let spec = Spec(Some(vec![]));
        let config = crate::config::Args::default();

        let mut value = serde_json::json!(TEST_G_ADDRESS);
        resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap();
        assert_eq!(value, serde_json::json!(TEST_G_ADDRESS));

        let mut value = serde_json::json!("bogus-alias");
        let err = resolve_aliases_in_json(&mut value, &ty, &spec, &config).unwrap_err();
        assert!(
            matches!(err, Error::Config(_) | Error::ScAddress(_)),
            "expected alias-resolution error, got {err:?}"
        );
    }

    #[test]
    fn resolve_aliases_preserves_input_when_nothing_mutated() {
        use stellar_xdr::curr::ScSpecTypeVec;

        // Type with no Address positions: input is returned verbatim,
        // including whitespace that compact JSON re-serialization would drop.
        let (spec, ty) = struct_spec(
            "Point",
            &[("x", ScSpecTypeDef::U32), ("y", ScSpecTypeDef::U32)],
        );
        let config = crate::config::Args::default();
        let pretty = r#"{ "x": 1, "y": 2 }"#;
        assert_eq!(
            resolve_aliases(pretty, &ty, &spec, &config).unwrap(),
            pretty
        );

        // Type with Address positions but no aliases: also returned verbatim.
        let ty = ScSpecTypeDef::Vec(Box::new(ScSpecTypeVec {
            element_type: Box::new(ScSpecTypeDef::Address),
        }));
        let spec = Spec(Some(vec![]));
        let pretty = format!(r#"[ "{TEST_G_ADDRESS}" ]"#);
        assert_eq!(
            resolve_aliases(&pretty, &ty, &spec, &config).unwrap(),
            pretty
        );
    }

    #[test]
    fn resolve_aliases_in_json_walks_map_keys() {
        use stellar_xdr::curr::ScSpecTypeMap;

        let map_ty = ScSpecTypeDef::Map(Box::new(ScSpecTypeMap {
            key_type: Box::new(ScSpecTypeDef::Address),
            value_type: Box::new(ScSpecTypeDef::U32),
        }));
        let spec = Spec(Some(vec![]));
        let config = crate::config::Args::default();

        let mut value = serde_json::json!({ TEST_G_ADDRESS: 1 });
        resolve_aliases_in_json(&mut value, &map_ty, &spec, &config).unwrap();
        assert_eq!(value, serde_json::json!({ TEST_G_ADDRESS: 1 }));

        let mut value = serde_json::json!({ "bogus-alias": 1 });
        let err = resolve_aliases_in_json(&mut value, &map_ty, &spec, &config).unwrap_err();
        assert!(
            matches!(err, Error::Config(_) | Error::ScAddress(_)),
            "expected alias-resolution error, got {err:?}"
        );
    }

    /// Mirrors `stellar contract invoke`: Spec::from_wasm -> build_clap_command -> render_long_help.
    #[test]
    fn invoke_help_strips_control_characters() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../crates/soroban-spec-tools/tests/fixtures/control_characters.wasm"
        );
        let bytes = std::fs::read(path).expect("fixture wasm should be readable");
        let spec = Spec::from_wasm(&bytes).expect("wasm should parse without error");
        let mut cmd = build_clap_command(&spec, true).expect("command should build without error");
        let help = cmd.render_long_help().to_string();

        let bad_chars: Vec<char> = help
            .chars()
            .filter(|c| c.is_control() && *c != '\n' && *c != '\t')
            .collect();
        assert!(
            bad_chars.is_empty(),
            "invoke help contains unexpected control characters {bad_chars:?}:\n{help:?}"
        );
    }
}
