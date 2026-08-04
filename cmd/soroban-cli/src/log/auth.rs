use std::fmt::Write;

use soroban_spec_tools::sanitize;

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
        SorobanCredentials::Address(creds) | SorobanCredentials::AddressV2(creds) => {
            let _ = writeln!(result, "    Signer: {}", format_address(&creds.address));
        }
        SorobanCredentials::AddressWithDelegates(creds) => {
            let _ = writeln!(
                result,
                "    Signer: {}",
                format_address(&creds.address_credentials.address)
            );
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
            let _ = writeln!(result, "{prefix}  Fn: {}", sanitize(fn_name));
            if !args.is_empty() {
                let _ = writeln!(result, "{prefix}  Args:");
                for arg in args {
                    let rendered = soroban_spec_tools::to_string(arg)
                        .unwrap_or(String::from("<unable to parse>"));
                    let _ = writeln!(result, "{prefix}    {}", sanitize(&rendered));
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
            for arg in args {
                let rendered =
                    soroban_spec_tools::to_string(arg).unwrap_or(String::from("<unable to parse>"));
                let _ = writeln!(result, "{prefix}      {}", sanitize(&rendered));
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
        ScAddress::Contract(stellar_xdr::ContractId(stellar_xdr::Hash(bytes))) => {
            format!(
                "{}",
                stellar_strkey::Strkey::Contract(stellar_strkey::Contract(*bytes))
            )
        }
        _ => format!("{address:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::ScSymbol;
    use soroban_spec_tools::test_utils::assert_no_control_chars;

    fn sc_symbol(s: &str) -> ScSymbol {
        ScSymbol(s.as_bytes().to_vec().try_into().unwrap())
    }

    // Auth entries come from the RPC's simulateTransaction response and are
    // rendered in the signing prompt, so a hostile RPC must not be able to
    // smuggle terminal-escape sequences through the function name or a
    // top-level `ScVal::Symbol` argument.
    #[test]
    fn format_auth_entry_strips_attacker_control_bytes() {
        let entry = SorobanAuthorizationEntry {
            credentials: SorobanCredentials::SourceAccount,
            root_invocation: SorobanAuthorizedInvocation {
                function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                    contract_address: ScAddress::Contract(stellar_xdr::ContractId(Hash([0; 32]))),
                    function_name: sc_symbol("\x1b[2Jhello"),
                    args: vec![ScVal::Symbol(sc_symbol("\x1b[31mworld"))]
                        .try_into()
                        .unwrap(),
                }),
                sub_invocations: VecM::default(),
            },
        };

        assert_no_control_chars(&format_auth_entry(&entry));
    }

    // `CreateContractV2` constructor args are rendered through a different code
    // path (`format_create_contract`) than the `ContractFn` case above, so it
    // needs its own guard: a hostile RPC must not smuggle terminal-escape
    // sequences through a constructor argument either.
    #[test]
    fn format_auth_entry_strips_control_bytes_from_constructor_args() {
        let entry = SorobanAuthorizationEntry {
            credentials: SorobanCredentials::SourceAccount,
            root_invocation: SorobanAuthorizedInvocation {
                function: SorobanAuthorizedFunction::CreateContractV2HostFn(CreateContractArgsV2 {
                    contract_id_preimage: ContractIdPreimage::Address(
                        ContractIdPreimageFromAddress {
                            address: ScAddress::Contract(stellar_xdr::ContractId(Hash([0; 32]))),
                            salt: Uint256([0; 32]),
                        },
                    ),
                    executable: ContractExecutable::Wasm(Hash([0; 32])),
                    constructor_args: vec![ScVal::Symbol(sc_symbol("\x1b[31mworld"))]
                        .try_into()
                        .unwrap(),
                }),
                sub_invocations: VecM::default(),
            },
        };

        assert_no_control_chars(&format_auth_entry(&entry));
    }
}
