use crate::xdr::{HostFunction, SorobanAuthorizedFunction, SorobanAuthorizedInvocation};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(
        "the produced signature does not match the cached public key {public_key}; \
the cached key may be stale — check the correct device is connected / the alias's \
hd-path is right, or re-add the key"
    )]
    SignatureMismatch { public_key: String },

    #[error("the cached public key {public_key} is not a valid ed25519 public key")]
    InvalidPublicKey { public_key: String },

    #[error("the signer returned a malformed ed25519 signature ({len} bytes, expected 64)")]
    InvalidSignature { len: usize },
}

/// Verify that `signature` over `message` was produced by the secret key
/// corresponding to `public_key`. Used as a post-sign drift guard for
/// cached-pubkey signers (Ledger / Secure Store): the signature is produced on a
/// live device/keychain while the hint and embedded public key are derived from a
/// cached value, so a stale cache yields a transaction the network silently
/// rejects. This catches that locally with a clear error.
pub fn verify_signature(
    public_key: &stellar_strkey::ed25519::PublicKey,
    message: &[u8; 32],
    signature: &[u8],
) -> Result<(), Error> {
    use ed25519_dalek::{Signature, VerifyingKey};
    let vk = VerifyingKey::from_bytes(&public_key.0).map_err(|_| Error::InvalidPublicKey {
        public_key: format!("{public_key}"),
    })?;
    let sig = Signature::from_slice(signature).map_err(|_| Error::InvalidSignature {
        len: signature.len(),
    })?;
    vk.verify_strict(message, &sig)
        .map_err(|_| Error::SignatureMismatch {
            public_key: format!("{public_key}"),
        })
}

/// Classification of an `Address`-credential auth entry's relationship to the
/// transaction's host function.
///
/// `SourceAccount` credential entries are out of scope here — they are signed
/// implicitly via the transaction envelope and never reach this classifier.
#[derive(Debug, PartialEq, Eq)]
pub enum AuthStyle {
    /// `root_invocation` matches the host function exactly. Safe to sign:
    /// the entry is bound to the host function.
    Strict,
    /// `root_invocation` does not match the host function exactly. Any transaction
    /// whose auth tree contains this entry could consume the resulting signature.
    NonStrict,
    /// `root_invocation` is not expected for the host function
    Invalid,
}

/// Classify an auth invocation against the transaction's host function.
///
/// ### Arguments
/// * `source_host_fn`- The transaction's host function
/// * `auth_invocation` - The auth entry's root invocation
pub fn classify_auth_invocation(
    source_host_fn: &HostFunction,
    auth_invocation: &SorobanAuthorizedInvocation,
) -> AuthStyle {
    // No auth entries are valid for `UploadContractWasm`.
    if matches!(source_host_fn, HostFunction::UploadContractWasm(_)) {
        return AuthStyle::Invalid;
    }

    // Check if the auth entry's root invocation matches the host function exactly.
    // This is different than just a `root_auth` check, as contracts that authorize with
    // `require_auth_for_args` at the root are not considered strict auth. This tradeoff is
    // made to ensure that even a tampered auth entry can be flagged as non-strict.
    let is_strict = match (source_host_fn, &auth_invocation.function) {
        (HostFunction::InvokeContract(op), SorobanAuthorizedFunction::ContractFn(args)) => {
            args == op
        }
        (
            HostFunction::CreateContract(op),
            SorobanAuthorizedFunction::CreateContractHostFn(args),
        ) => args == op,
        (
            HostFunction::CreateContractV2(op),
            SorobanAuthorizedFunction::CreateContractV2HostFn(args),
        ) => args == op,
        _ => false,
    };

    if is_strict {
        AuthStyle::Strict
    } else {
        AuthStyle::NonStrict
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        AccountId, BytesM, ContractExecutable, ContractIdPreimage, ContractIdPreimageFromAddress,
        CreateContractArgsV2, Hash, InvokeContractArgs, PublicKey, ScAddress, ScSymbol, ScVal,
        Uint256, VecM,
    };
    use stellar_strkey::ed25519;

    const SOURCE_ACCOUNT: &str = "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI";

    fn source_bytes() -> [u8; 32] {
        ed25519::PublicKey::from_string(SOURCE_ACCOUNT).unwrap().0
    }

    fn ed25519_address(bytes: [u8; 32]) -> ScAddress {
        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(bytes))))
    }

    fn host_fn_invoke(contract: [u8; 32], fn_name: &str, args: &[ScVal]) -> HostFunction {
        HostFunction::InvokeContract(InvokeContractArgs {
            contract_address: ScAddress::Contract(stellar_xdr::ContractId(Hash(contract))),
            function_name: ScSymbol(fn_name.try_into().unwrap()),
            args: args.try_into().unwrap(),
        })
    }

    fn host_fn_create(wasm_hash: [u8; 32], args: &[ScVal]) -> HostFunction {
        HostFunction::CreateContractV2(CreateContractArgsV2 {
            contract_id_preimage: ContractIdPreimage::Address(ContractIdPreimageFromAddress {
                address: ed25519_address(source_bytes()),
                salt: Uint256([0u8; 32]),
            }),
            executable: ContractExecutable::Wasm(wasm_hash.into()),
            constructor_args: args.try_into().unwrap(),
        })
    }

    fn invocation_contract(
        contract: [u8; 32],
        fn_name: &str,
        args: &[ScVal],
    ) -> SorobanAuthorizedInvocation {
        SorobanAuthorizedInvocation {
            function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                contract_address: ScAddress::Contract(stellar_xdr::ContractId(Hash(contract))),
                function_name: ScSymbol(fn_name.try_into().unwrap()),
                args: args.to_vec().try_into().unwrap(),
            }),
            sub_invocations: VecM::default(),
        }
    }

    fn invocation_create(wasm_hash: [u8; 32], args: &[ScVal]) -> SorobanAuthorizedInvocation {
        SorobanAuthorizedInvocation {
            function: SorobanAuthorizedFunction::CreateContractV2HostFn(CreateContractArgsV2 {
                contract_id_preimage: ContractIdPreimage::Address(ContractIdPreimageFromAddress {
                    address: ed25519_address(source_bytes()),
                    salt: Uint256([0u8; 32]),
                }),
                executable: ContractExecutable::Wasm(wasm_hash.into()),
                constructor_args: args.try_into().unwrap(),
            }),
            sub_invocations: VecM::default(),
        }
    }

    #[test]
    fn test_verify_signature_roundtrip() {
        use ed25519_dalek::{ed25519::signature::Signer as _, SigningKey};

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let public_key =
            ed25519::PublicKey::from_payload(signing_key.verifying_key().as_bytes()).unwrap();
        let message = [42u8; 32];
        let signature = signing_key.sign(&message).to_bytes();

        // Matching key + untampered signature verifies.
        verify_signature(&public_key, &message, &signature).unwrap();

        // A different public key is rejected.
        let cached_pk = ed25519::PublicKey::from_payload(
            SigningKey::from_bytes(&[9u8; 32])
                .verifying_key()
                .as_bytes(),
        )
        .unwrap();
        assert!(matches!(
            verify_signature(&cached_pk, &message, &signature),
            Err(Error::SignatureMismatch { .. })
        ));

        // A tampered signature is rejected.
        let mut tampered = signature;
        tampered[0] ^= 0xff;
        assert!(matches!(
            verify_signature(&public_key, &message, &tampered),
            Err(Error::SignatureMismatch { .. })
        ));

        // A wrong-length signature is reported as malformed, not as a mismatch.
        assert!(matches!(
            verify_signature(&public_key, &message, &signature[..63]),
            Err(Error::InvalidSignature { len: 63 })
        ));
    }

    #[test]
    fn test_matching_root_invocation_is_strict() {
        let contract = [1u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = host_fn_invoke(contract, "hello", args);
        let invocation = invocation_contract(contract, "hello", args);

        let style = classify_auth_invocation(&host_fn, &invocation);
        assert_eq!(style, AuthStyle::Strict);
    }

    #[test]
    fn test_subinvocations_dont_affect_root_match() {
        let contract = [1u8; 32];
        let other = [99u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = host_fn_invoke(contract, "hello", args);
        let mut invocation = invocation_contract(contract, "hello", args);
        invocation.sub_invocations = [invocation_contract(other, "other", &[])]
            .try_into()
            .unwrap();

        let style = classify_auth_invocation(&host_fn, &invocation);
        assert_eq!(style, AuthStyle::Strict);
    }

    #[test]
    fn test_different_root_contract_is_non_strict() {
        let contract = [1u8; 32];
        let other = [99u8; 32];

        let host_fn = host_fn_invoke(contract, "hello", &[]);
        let invocation = invocation_contract(other, "hello", &[]);

        let style = classify_auth_invocation(&host_fn, &invocation);
        assert_eq!(style, AuthStyle::NonStrict);
    }

    #[test]
    fn test_different_function_same_contract_is_non_strict() {
        let contract = [1u8; 32];

        let host_fn = host_fn_invoke(contract, "hello", &[]);
        let invocation = invocation_contract(contract, "transfer", &[]);

        let style = classify_auth_invocation(&host_fn, &invocation);
        assert_eq!(style, AuthStyle::NonStrict);
    }

    #[test]
    fn test_different_args_is_non_strict() {
        let contract = [1u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];
        let wrong = &[ScVal::U32(43), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = host_fn_invoke(contract, "hello", args);
        let invocation = invocation_contract(contract, "hello", wrong);

        let style = classify_auth_invocation(&host_fn, &invocation);
        assert_eq!(style, AuthStyle::NonStrict);
    }

    #[test]
    fn test_upload_wasm_with_auth_entry_is_invalid() {
        let contract = [1u8; 32];
        let wasm_hash: BytesM = [42u8; 32].try_into().unwrap();

        let host_fn = HostFunction::UploadContractWasm(wasm_hash);
        let invocation = invocation_contract(contract, "hello", &[]);

        let style = classify_auth_invocation(&host_fn, &invocation);
        assert_eq!(style, AuthStyle::Invalid);
    }

    #[test]
    fn test_matching_create_contract_root_is_strict() {
        let contract = [1u8; 32];
        let wasm_hash = [42u8; 32];
        let args = &[ScVal::U32(42), ScVal::Symbol("hello".try_into().unwrap())];

        let host_fn = host_fn_create(wasm_hash, args);
        let mut invocation = invocation_create(wasm_hash, args);
        invocation.sub_invocations = [invocation_contract(contract, "__constructor", args)]
            .try_into()
            .unwrap();

        let style = classify_auth_invocation(&host_fn, &invocation);
        assert_eq!(style, AuthStyle::Strict);
    }
}
