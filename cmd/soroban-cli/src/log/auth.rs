use std::fmt::Write;

use crate::xdr::{
    AccountId, ContractExecutable, ContractIdPreimage, ContractIdPreimageFromAddress,
    CreateContractArgs, CreateContractArgsV2, Hash, InvokeContractArgs, PublicKey, ScAddress,
    ScVal, SorobanAuthorizationEntry, SorobanAuthorizedFunction, SorobanAuthorizedInvocation,
    SorobanCredentials, Uint256, VecM,
};

/// Format a single auth entry for display.
pub fn format_auth_entry(entry: &SorobanAuthorizationEntry) -> String {
    let mut result = String::from("  Auth Entry:\n");

    match &entry.credentials {
        SorobanCredentials::Address(creds) => {
            let _ = writeln!(result, "    Signer: {}", format_address(&creds.address));
        }
        SorobanCredentials::SourceAccount => {
            result.push_str("    Signer: <source account>\n");
        }
    }

    format_invocation(&entry.root_invocation, 2, "Invocation:", &mut result);

    result
}

/// Recursively format a `SorobanAuthorizedInvocation` tree. `label` is the
/// header line printed for this node — `"Invocation:"` for the root and
/// `"Sub-invocation #N:"` for each child.
fn format_invocation(
    invocation: &SorobanAuthorizedInvocation,
    indent: usize,
    label: &str,
    result: &mut String,
) {
    let prefix = "  ".repeat(indent);
    let _ = writeln!(result, "{prefix}{label}");

    match &invocation.function {
        SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
            contract_address,
            function_name,
            args,
        }) => {
            let fn_name = std::str::from_utf8(function_name.as_ref()).unwrap_or("<invalid>");
            let _ = writeln!(
                result,
                "{prefix}  Contract: {}",
                format_address(contract_address)
            );
            let _ = writeln!(result, "{prefix}  Fn: {fn_name}");
            if !args.is_empty() {
                let _ = writeln!(result, "{prefix}  Args:");
                for arg in args.iter() {
                    let _ = writeln!(
                        result,
                        "{prefix}    {}",
                        soroban_spec_tools::to_string(arg)
                            .unwrap_or(String::from("<unable to parse>"))
                    );
                }
            }
        }
        SorobanAuthorizedFunction::CreateContractHostFn(CreateContractArgs {
            contract_id_preimage,
            executable,
        }) => {
            let _ = writeln!(result, "{prefix}  CreateContract");
            format_create_contract(contract_id_preimage, executable, None, &prefix, result);
        }
        SorobanAuthorizedFunction::CreateContractV2HostFn(CreateContractArgsV2 {
            contract_id_preimage,
            executable,
            constructor_args,
        }) => {
            let _ = writeln!(result, "{prefix}  CreateContractV2");
            format_create_contract(
                contract_id_preimage,
                executable,
                Some(constructor_args),
                &prefix,
                result,
            );
        }
    }

    for (i, sub) in invocation.sub_invocations.iter().enumerate() {
        let sub_label = format!("Sub-invocation #{i}:");
        format_invocation(sub, indent + 1, &sub_label, result);
    }
}

/// Format the body of a `CreateContract` / `CreateContractV2` auth entry: the
/// id preimage (source + salt, or asset), the executable (wasm hash or
/// stellar asset), and — for V2 — any constructor args. Indented two levels
/// below `prefix` so it sits under the `CreateContract` header line.
fn format_create_contract(
    preimage: &ContractIdPreimage,
    executable: &ContractExecutable,
    constructor_args: Option<&VecM<ScVal>>,
    prefix: &str,
    result: &mut String,
) {
    match preimage {
        ContractIdPreimage::Address(ContractIdPreimageFromAddress {
            address,
            salt: Uint256(salt_bytes),
        }) => {
            let _ = writeln!(result, "{prefix}    From: {}", format_address(address));
            let _ = writeln!(result, "{prefix}    Salt: {}", hex::encode(salt_bytes));
        }
        ContractIdPreimage::Asset(asset) => {
            let _ = writeln!(result, "{prefix}    Asset: {asset:?}");
        }
    }
    match executable {
        ContractExecutable::Wasm(Hash(bytes)) => {
            let _ = writeln!(result, "{prefix}    Wasm: {}", hex::encode(bytes));
        }
        ContractExecutable::StellarAsset => {
            let _ = writeln!(result, "{prefix}    Executable: StellarAsset");
        }
    }
    if let Some(args) = constructor_args {
        if !args.is_empty() {
            let _ = writeln!(result, "{prefix}    Constructor Args:");
            for arg in args.iter() {
                let _ = writeln!(
                    result,
                    "{prefix}      {}",
                    soroban_spec_tools::to_string(arg).unwrap_or(String::from("<unable to parse>"))
                );
            }
        }
    }
}

/// Format an ScAddress as a strkey string for display.
fn format_address(address: &ScAddress) -> String {
    match address {
        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(bytes)))) => {
            format!(
                "{}",
                stellar_strkey::Strkey::PublicKeyEd25519(stellar_strkey::ed25519::PublicKey(
                    *bytes
                ))
            )
        }
        ScAddress::Contract(stellar_xdr::curr::ContractId(stellar_xdr::curr::Hash(bytes))) => {
            format!(
                "{}",
                stellar_strkey::Strkey::Contract(stellar_strkey::Contract(*bytes))
            )
        }
        _ => format!("{address:?}"),
    }
}
