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
use soroban_spec_tools::Spec;
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

pub async fn build_host_function_parameters(
    contract_id: &stellar_strkey::Contract,
    slop: &[OsString],
    spec_entries: &[ScSpecEntry],
    config: &config::Args,
) -> Result<HostFunctionParameters, Error> {
    build_host_function_parameters_with_filter(contract_id, slop, spec_entries, config, true).await
}

pub async fn build_constructor_parameters(
    contract_id: &stellar_strkey::Contract,
    slop: &[OsString],
    spec_entries: &[ScSpecEntry],
    config: &config::Args,
) -> Result<HostFunctionParameters, Error> {
    build_host_function_parameters_with_filter(contract_id, slop, spec_entries, config, false).await
}

async fn build_host_function_parameters_with_filter(
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
    let (parsed_args, signers) = parse_function_arguments(&func, &matches_, &spec, config).await?;
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
    spec.find_function(function)
        .map_err(|_| Error::FunctionNotFoundInContractSpec {
            function_name: function.to_string(),
            available_functions: get_available_functions(spec),
        })
        .cloned()
}

async fn parse_function_arguments(
    func: &ScSpecFunctionV0,
    matches_: &clap::ArgMatches,
    spec: &Spec,
    config: &config::Args,
) -> Result<(Vec<ScVal>, Vec<Signer>), Error> {
    let mut parsed_args = Vec::with_capacity(func.inputs.len());
    let mut signers = Vec::<Signer>::new();

    for i in func.inputs.iter() {
        parse_single_argument(i, matches_, spec, config, &mut signers, &mut parsed_args).await?;
    }

    Ok((parsed_args, signers))
}

async fn parse_single_argument(
    input: &stellar_xdr::curr::ScSpecFunctionInputV0,
    matches_: &clap::ArgMatches,
    spec: &Spec,
    config: &config::Args,
    signers: &mut Vec<Signer>,
    parsed_args: &mut Vec<ScVal>,
) -> Result<(), Error> {
    let name = input.name.to_utf8_string()?;
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

        // Handle address types with signer resolution
        if matches!(
            input.type_,
            ScSpecTypeDef::Address | ScSpecTypeDef::MuxedAddress
        ) {
            let trimmed_s = s.trim_matches('"');
            let addr = resolve_address(trimmed_s, config).await?;

            let signer = resolve_signer(&s, config).await;
                if let Some(signer) = signer {
                    signers.push(signer);
                }

            parsed_args.push(parse_argument_with_validation(
                &name,
                &addr,
                &input.type_,
                spec,
                config,
            ).await?);
            return Ok(());
        }

        parsed_args.push(parse_argument_with_validation(
            &name,
            &s,
            &input.type_,
            spec,
            config,
        ).await?);
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
        ).await?);
        Ok(())
    } else {
        Err(Error::MissingArgument {
            arg: name,
            expected_type: expected_type_name,
        })
    }
}

async fn parse_file_argument(
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
        parse_argument_with_validation(name, &file_contents, type_def, spec, config).await
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
        .map(|i| (i.name.to_utf8_string().unwrap(), i.type_.clone()))
        .collect::<HashMap<String, ScSpecTypeDef>>();
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let mut cmd = clap::Command::new(name)
        .no_binary_name(true)
        .term_width(300)
        .max_term_width(300);
    let kebab_name = name.to_kebab_case();
    if kebab_name != name {
        cmd = cmd.alias(kebab_name);
    }
    let doc: &'static str = Box::leak(func.doc.to_utf8_string_lossy().into_boxed_str());
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
            .long_help(spec.doc(name, type_)?);

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

async fn resolve_address(addr_or_alias: &str, config: &config::Args) -> Result<String, Error> {
    let sc_address: UnresolvedScAddress = addr_or_alias.parse().unwrap();
    let account = match sc_address {
        UnresolvedScAddress::Resolved(addr) => addr.to_string(),
        addr @ UnresolvedScAddress::Alias(_) => {
            let addr = addr.resolve_async(&config.locator, &config.get_network()?.network_passphrase).await?;
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

async fn resolve_signer(addr_or_alias: &str, config: &config::Args) -> Option<Signer> {
    let secret = config.locator.get_secret_key(addr_or_alias).ok()?;
    let print = Print::new(false);
    let signer = secret.signer(None, print).await.ok()?;
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
            format!("user-defined type '{}'", udt.name.to_utf8_string_lossy())
        }
    }
}

/// Gets available function names for error messages
fn get_available_functions(spec: &Spec) -> String {
    match spec.find_functions() {
        Ok(functions) => functions
            .map(|f| f.name.to_utf8_string_lossy())
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
async fn parse_argument_with_validation(
    arg_name: &str,
    value: &str,
    expected_type: &ScSpecTypeDef,
    spec: &Spec,
    config: &config::Args,
) -> Result<ScVal, Error> {
    let expected_type_name = get_type_name(expected_type);

    // Pre-validate JSON for non-primitive types
    if !is_primitive_type(expected_type) {
        validate_json_arg(arg_name, value)?;
    }

    // Handle special address types
    if matches!(
        expected_type,
        ScSpecTypeDef::Address | ScSpecTypeDef::MuxedAddress
    ) {
        let trimmed_value = value.trim_matches('"');
        let addr = resolve_address(trimmed_value, config).await?;
        return spec
            .from_string(&addr, expected_type)
            .map_err(|error| Error::CannotParseArg {
                arg: arg_name.to_string(),
                error,
                expected_type: expected_type_name.clone(),
                received_value: value.to_string(),
                suggestion: get_context_suggestions(expected_type, value),
            });
    }

    // Parse the argument
    spec.from_string(value, expected_type)
        .map_err(|error| Error::CannotParseArg {
            arg: arg_name.to_string(),
            error,
            expected_type: expected_type_name,
            received_value: value.to_string(),
            suggestion: get_context_suggestions(expected_type, value),
        })
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
}
