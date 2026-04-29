use std::fmt::Write;

use crate::xdr::{
    AccountId, InvokeContractArgs, PublicKey, ScAddress, SorobanAuthorizationEntry,
    SorobanAuthorizedFunction, SorobanAuthorizedInvocation, SorobanCredentials, Uint256,
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
        SorobanAuthorizedFunction::CreateContractHostFn(_)
        | SorobanAuthorizedFunction::CreateContractV2HostFn(_) => {
            let _ = writeln!(result, "{prefix}  CreateContract");
        }
    }

    for (i, sub) in invocation.sub_invocations.iter().enumerate() {
        let sub_label = format!("Sub-invocation #{i}:");
        format_invocation(sub, indent + 1, &sub_label, result);
    }
}

/// Format an ScAddress as a strkey string for display.
fn format_address(address: &ScAddress) -> String {
    match address {
        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(bytes)))) => {
            stellar_strkey::Strkey::PublicKeyEd25519(stellar_strkey::ed25519::PublicKey(*bytes))
                .to_string()
        }
        ScAddress::Contract(stellar_xdr::curr::ContractId(stellar_xdr::curr::Hash(bytes))) => {
            stellar_strkey::Strkey::Contract(stellar_strkey::Contract(*bytes)).to_string()
        }
        _ => format!("{address:?}"),
    }
}
