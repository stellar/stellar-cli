use crate::{
    log::{auth, format_auth_entry},
    print, signer,
    xdr::{
        AccountId, HostFunction, InvokeContractArgs, InvokeHostFunctionOp, MuxedAccount, Operation,
        OperationBody, PublicKey, ScAddress, SorobanAuthorizationEntry, SorobanAuthorizedFunction,
        SorobanCredentials, Transaction, Uint256,
    },
};

/// Check transactions for any malformed or non-strict authorization entries. These entries, if signed,
/// could be submitted outside of the context of the transaction's contract invocation. Note that
/// this function does not protect against interacting with a malicious contract, and should not
/// be relied on as protection against potentially malicious transactions.
///
/// Enforces two checks:
///
/// 1. **Source account uses SourceAccount credentials**
///    The source account's auth should use `SourceAccount` credentials. If it
///    appears as `Address`, the simulation's auth recording is not correct.
///
/// 2. **Auth entry root invocation matches the tx operation**
///    Auth entries with `Address` credentials whose root invocation doesn't match the tx's
///    InvokeHostFunction could be used outside of the context of the transaction.
///    This does impact contracts that use `require_auth_for_args` as there is no way
///    to verify the authorization entry can't be recreated with different function arguments.
///
/// This function also logs all auth entries that are caught by those two flags.
///
/// # Errors
/// * If the source account credential is used directly instead of as a `SourceAccount` credential.
/// * If non-root auth is detected
pub fn check_auth(tx: &Transaction, quiet: bool) -> Result<(), signer::Error> {
    let print = print::Print::new(quiet);
    let source_bytes = source_account_bytes(tx);
    // only need to check auth if op is `InvokeHostFunction`
    let Some(invoke_host_op) = get_op(tx) else {
        return Ok(());
    };

    let mut non_strict_entries: Vec<&SorobanAuthorizationEntry> = Vec::new();
    for entry in invoke_host_op.auths() {
        let SorobanCredentials::Address(ref creds) = entry.credentials else {
            // SourceAccount credential entries are not signed explicitly
            // so there is no risk of them being used outside the context of the transaction.
            continue;
        };

        // Check if source account appears as Address credential
        if let Some(auth_addr) = auth_address_bytes(&creds.address) {
            if source_bytes == auth_addr {
                print.warnln("Source account detected with Address credentials. This requires an extra signature and is not expected.");
                print.warnln(format_auth_entry(entry));
                return Err(signer::Error::InvalidAuthEntry);
            }
        }

        // Check if the auth entry is strict. That is, it cannot be submitted outside the
        // context of the transaction's host function. This check is overly strict as it doesn't
        // allow for the usage of `require_auth_for_args`.
        let is_strict = match &invoke_host_op.host_function {
            // For `InvokeContract`, validate the root invocation matches the host function arguments.
            HostFunction::InvokeContract(op) => match &entry.root_invocation.function {
                SorobanAuthorizedFunction::ContractFn(auth_args) => auth_args == op,
                _ => false,
            },
            // For `CreateContract` and `CreateContractV2`, the root invocation should
            // match the host function arguments.
            HostFunction::CreateContract(op) => match &entry.root_invocation.function {
                SorobanAuthorizedFunction::CreateContractHostFn(auth_args) => auth_args == op,
                _ => false,
            },
            HostFunction::CreateContractV2(op) => match &entry.root_invocation.function {
                SorobanAuthorizedFunction::CreateContractV2HostFn(auth_args) => auth_args == op,
                _ => false,
            },
            // auth entries shouldn't exist for other host functions
            HostFunction::UploadContractWasm(_) => {
                print.warnln(format!(
                    "Auth entry not expected for the host function {}",
                    invoke_host_op.host_function.name()
                ));
                print.warnln(auth::format_auth_entry(entry));
                return Err(signer::Error::InvalidAuthEntry);
            }
        };

        if !is_strict {
            non_strict_entries.push(entry);
        }
    }

    if non_strict_entries.is_empty() {
        Ok(())
    } else {
        print.warnln(
            "Authorization entries detected that could be submitted outside the context of this transaction:",
        );
        for entry in non_strict_entries {
            print.println(format_auth_entry(entry));
        }
        Err(signer::Error::OutOfContextAuthEntry)
    }
}

/// Extract the Ed25519 public key bytes from a transaction's source account.
fn source_account_bytes(tx: &Transaction) -> [u8; 32] {
    match &tx.source_account {
        MuxedAccount::Ed25519(Uint256(bytes)) => *bytes,
        MuxedAccount::MuxedEd25519(muxed) => muxed.ed25519.0,
    }
}

/// Extract the Ed25519 public key bytes from an auth entry's credential address.
fn auth_address_bytes(address: &ScAddress) -> Option<[u8; 32]> {
    match address {
        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(bytes)))) => {
            Some(*bytes)
        }
        _ => None,
    }
}

/// Extract the host function from a transaction, if it's an InvokeHostFunction operation.
fn get_op(tx: &Transaction) -> Option<&InvokeHostFunctionOp> {
    let [Operation {
        body: OperationBody::InvokeHostFunction(invoke_host_function_op),
        ..
    }] = tx.operations.as_slice()
    else {
        return None;
    };
    Some(invoke_host_function_op)
}

/// Extract the function name from a root invocation, if it's a contract function call.
pub fn invocation_function_name(auth: &SorobanAuthorizationEntry) -> Option<String> {
    match &auth.root_invocation.function {
        SorobanAuthorizedFunction::ContractFn(InvokeContractArgs { function_name, .. }) => {
            std::str::from_utf8(function_name.as_ref())
                .ok()
                .map(String::from)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        BytesM, ContractExecutable, ContractIdPreimage, ContractIdPreimageFromAddress,
        CreateContractArgsV2, Hash, InvokeContractArgs, InvokeHostFunctionOp, Memo, Preconditions,
        ScSymbol, ScVal, SequenceNumber, SorobanAddressCredentials, SorobanAuthorizedFunction,
        SorobanAuthorizedInvocation, TransactionExt, VecM,
    };
    use stellar_strkey::ed25519;

    const SOURCE_ACCOUNT: &str = "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI";
    const OTHER_ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    fn source_account_bytes() -> [u8; 32] {
        ed25519::PublicKey::from_string(SOURCE_ACCOUNT).unwrap().0
    }

    fn other_account_bytes() -> [u8; 32] {
        ed25519::PublicKey::from_string(OTHER_ACCOUNT).unwrap().0
    }

    fn test_transaction(
        host_fn: &HostFunction,
        auth_entries: &[SorobanAuthorizationEntry],
    ) -> Transaction {
        Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(source_account_bytes())),
            fee: 100,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function: host_fn.clone(),
                    auth: auth_entries.to_vec().try_into().unwrap(),
                }),
            }]
            .try_into()
            .unwrap(),
            ext: TransactionExt::V0,
        }
    }

    fn make_host_fn_invoke_contract(
        contract_addr: [u8; 32],
        fn_name: &str,
        args: &[ScVal],
    ) -> HostFunction {
        HostFunction::InvokeContract(InvokeContractArgs {
            contract_address: ScAddress::Contract(stellar_xdr::curr::ContractId(Hash(
                contract_addr,
            ))),
            function_name: ScSymbol(fn_name.try_into().unwrap()),
            args: args.try_into().unwrap(),
        })
    }

    fn make_host_fn_create_contract(wasm_hash: [u8; 32], args: &[ScVal]) -> HostFunction {
        HostFunction::CreateContractV2(CreateContractArgsV2 {
            contract_id_preimage: ContractIdPreimage::Address(ContractIdPreimageFromAddress {
                address: ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
                    source_account_bytes(),
                )))),
                salt: Uint256([0u8; 32]),
            }),
            executable: ContractExecutable::Wasm(wasm_hash.into()),
            constructor_args: args.try_into().unwrap(),
        })
    }

    fn make_auth_entry(
        address_bytes: [u8; 32],
        invocation: &SorobanAuthorizedInvocation,
    ) -> SorobanAuthorizationEntry {
        SorobanAuthorizationEntry {
            credentials: SorobanCredentials::Address(SorobanAddressCredentials {
                address: ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
                    address_bytes,
                )))),
                nonce: 0,
                signature_expiration_ledger: 0,
                signature: ScVal::Void,
            }),
            root_invocation: invocation.clone(),
        }
    }

    fn make_source_account_auth_entry(
        invocation: &SorobanAuthorizedInvocation,
    ) -> SorobanAuthorizationEntry {
        SorobanAuthorizationEntry {
            credentials: SorobanCredentials::SourceAccount,
            root_invocation: invocation.clone(),
        }
    }

    fn make_auth_invocation_contract(
        contract_addr: [u8; 32],
        fn_name: &str,
        args: &[ScVal],
    ) -> SorobanAuthorizedInvocation {
        SorobanAuthorizedInvocation {
            function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                contract_address: ScAddress::Contract(stellar_xdr::curr::ContractId(Hash(
                    contract_addr,
                ))),
                function_name: ScSymbol(fn_name.try_into().unwrap()),
                args: args.to_vec().try_into().unwrap(),
            }),
            sub_invocations: VecM::default(),
        }
    }

    fn make_auth_invocation_create_contract(
        wasm_hash: [u8; 32],
        args: &[ScVal],
    ) -> SorobanAuthorizedInvocation {
        SorobanAuthorizedInvocation {
            function: SorobanAuthorizedFunction::CreateContractV2HostFn(CreateContractArgsV2 {
                contract_id_preimage: ContractIdPreimage::Address(ContractIdPreimageFromAddress {
                    address: ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(
                        Uint256(source_account_bytes()),
                    ))),
                    salt: Uint256([0u8; 32]),
                }),
                executable: ContractExecutable::Wasm(wasm_hash.into()),
                constructor_args: args.try_into().unwrap(),
            }),
            sub_invocations: VecM::default(),
        }
    }

    #[test]
    fn test_matching_root_invocation_passes() {
        let contract = [1u8; 32];
        let other_contract = [99u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = make_host_fn_invoke_contract(contract, "hello", args);
        let mut invocation = make_auth_invocation_contract(contract, "hello", args);
        let sub_invocation = make_auth_invocation_contract(other_contract, "other", &[]);
        invocation.sub_invocations = [sub_invocation].try_into().unwrap();
        let auth = make_auth_entry(other_account_bytes(), &invocation);
        let tx = test_transaction(&host_fn, &[auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_matching_root_invocation_with_subinvocations_passes() {
        let contract = [1u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = make_host_fn_invoke_contract(contract, "hello", args);
        let invocation = make_auth_invocation_contract(contract, "hello", args);
        let auth = make_auth_entry(other_account_bytes(), &invocation);
        let tx = test_transaction(&host_fn, &[auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_source_account_as_address_credential_is_rejected() {
        let contract = [1u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = make_host_fn_invoke_contract(contract, "hello", args);
        let invocation = make_auth_invocation_contract(contract, "hello", args);
        let auth = make_auth_entry(source_account_bytes(), &invocation);
        let tx = test_transaction(&host_fn, &[auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_source_account_credentials_passes() {
        let contract = [1u8; 32];
        let other_contract = [99u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = make_host_fn_invoke_contract(contract, "hello", args);
        let invocation = make_auth_invocation_contract(other_contract, "other", &[]);
        let auth = make_source_account_auth_entry(&invocation);
        let tx = test_transaction(&host_fn, &[auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_non_root_auth_is_rejected() {
        let contract = [1u8; 32];
        let other_contract = [99u8; 32];

        let host_fn = make_host_fn_invoke_contract(contract, "hello", &[]);
        let invocation = make_auth_invocation_contract(other_contract, "hello", &[]);
        let auth = make_auth_entry(other_account_bytes(), &invocation);
        let tx = test_transaction(&host_fn, &[auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_different_function_same_contract_is_rejected() {
        let contract = [1u8; 32];

        let host_fn = make_host_fn_invoke_contract(contract, "hello", &[]);
        let invocation = make_auth_invocation_contract(contract, "transfer", &[]);
        let auth = make_auth_entry(other_account_bytes(), &invocation);
        let tx = test_transaction(&host_fn, &[auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_different_args_is_rejected() {
        let contract = [1u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];
        let wrong_args = &[ScVal::U32(43), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = make_host_fn_invoke_contract(contract, "hello", args);
        let invocation = make_auth_invocation_contract(contract, "hello", wrong_args);
        let auth = make_auth_entry(other_account_bytes(), &invocation);
        let tx = test_transaction(&host_fn, &[auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_valid_passes() {
        let contract = [1u8; 32];
        let other_contract = [99u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = make_host_fn_invoke_contract(contract, "hello", args);

        // Root-matching auth — safe
        let root_invocation = make_auth_invocation_contract(contract, "hello", args);
        let safe_auth = make_auth_entry(other_account_bytes(), &root_invocation);

        // SourceAccount auth for different contract — safe
        let other_invocation = make_auth_invocation_contract(other_contract, "anything", &[]);
        let source_auth = make_source_account_auth_entry(&other_invocation);

        // Root auth for different contract
        let other_invocation = make_auth_invocation_contract(contract, "hello", args);
        let other_auth = make_auth_entry(other_account_bytes(), &other_invocation);

        let tx = test_transaction(&host_fn, &[safe_auth, source_auth, other_auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rejects_if_any_fail_check() {
        let contract = [1u8; 32];
        let other_contract = [99u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = make_host_fn_invoke_contract(contract, "hello", args);

        // Root-matching auth — safe
        let root_invocation = make_auth_invocation_contract(contract, "hello", args);
        let safe_auth = make_auth_entry(other_account_bytes(), &root_invocation);

        // SourceAccount auth for different contract — safe
        let other_invocation = make_auth_invocation_contract(other_contract, "anything", &[]);
        let source_auth = make_source_account_auth_entry(&other_invocation);

        // Non-Root auth for different contract
        let other_invocation = make_auth_invocation_contract(other_contract, "hello", args);
        let other_auth = make_auth_entry(other_account_bytes(), &other_invocation);

        let tx = test_transaction(&host_fn, &[safe_auth, source_auth, other_auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_upload_wasm_host_function_passes() {
        let wasm_hash: BytesM = [42u8; 32].try_into().unwrap();

        let host_fn = HostFunction::UploadContractWasm(wasm_hash);
        let tx = test_transaction(&host_fn, &[]);

        let result = check_auth(&tx, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_upload_wasm_host_function_with_auth_is_rejected() {
        let contract = [1u8; 32];
        let wasm_hash: BytesM = [42u8; 32].try_into().unwrap();

        let host_fn = HostFunction::UploadContractWasm(wasm_hash);
        let invocation = make_auth_invocation_contract(contract, "hello", &[]);
        let auth = make_auth_entry(other_account_bytes(), &invocation);
        let tx = test_transaction(&host_fn, &[auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_deploy_no_constructor_passes() {
        let wasm_hash = [42u8; 32];

        let host_fn = make_host_fn_create_contract(wasm_hash, &[]);
        let tx = test_transaction(&host_fn, &[]);

        let result = check_auth(&tx, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_deploy_constructor_passes() {
        let contract = [1u8; 32];
        let wasm_hash = [42u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = make_host_fn_create_contract(wasm_hash, args);
        let mut invocation = make_auth_invocation_create_contract(wasm_hash, args);
        let sub_invocation = make_auth_invocation_contract(contract, "__constructor", args);
        invocation.sub_invocations = [sub_invocation].try_into().unwrap();
        let auth = make_auth_entry(other_account_bytes(), &invocation);
        let tx = test_transaction(&host_fn, &[auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_deploy_constructor_with_non_source_auth() {
        let contract = [1u8; 32];
        let wasm_hash = [42u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = make_host_fn_create_contract(wasm_hash, args);
        let invocation = make_auth_invocation_create_contract(wasm_hash, args);
        let auth = make_auth_entry(source_account_bytes(), &invocation);
        let other_invocation = make_auth_invocation_contract(contract, "__constructor", args);
        let other_auth = make_auth_entry(other_account_bytes(), &other_invocation);
        let tx = test_transaction(&host_fn, &[auth, other_auth]);

        let result = check_auth(&tx, true);
        assert!(result.is_err());
    }
}
