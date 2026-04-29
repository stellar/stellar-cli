use crate::{
    log::format_auth_entry,
    utils::fee_bump_transaction_hash,
    xdr::{
        self, AccountId, DecoratedSignature, FeeBumpTransactionEnvelope, Hash, HashIdPreimage,
        HashIdPreimageSorobanAuthorization, Limits, MuxedAccount, Operation, OperationBody,
        PublicKey, ScAddress, ScMap, ScSymbol, ScVal, Signature, SignatureHint,
        SorobanAddressCredentials, SorobanAuthorizationEntry, SorobanCredentials, Transaction,
        TransactionEnvelope, TransactionV1Envelope, Uint256, VecM, WriteXdr,
    },
};
use ed25519_dalek::{ed25519::signature::Signer as _, Signature as Ed25519Signature};
use sha2::{Digest, Sha256};

use crate::{config::network::Network, print::Print, utils::transaction_hash};

pub mod ledger;
pub mod validation;

#[cfg(feature = "additional-libs")]
mod keyring;
pub mod secure_store;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Contract addresses are not supported to sign auth entries {address}")]
    ContractAddressAreNotSupported { address: String },
    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("Missing signing key for account {address}")]
    MissingSignerForAddress { address: String },
    #[error(transparent)]
    TryFromSlice(#[from] std::array::TryFromSliceError),
    #[error("Signing authorization entries that could be submitted outside the context of the transaction is not supported in the CLI:\n{auth_entry_str}")]
    NotStrictAuthEntry { auth_entry_str: String },
    #[error("Invalid Soroban authorization entry - {reason}:\n{auth_entry_str}")]
    InvalidAuthEntry {
        reason: String,
        auth_entry_str: String,
    },
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("Transaction envelope type not supported")]
    UnsupportedTransactionEnvelopeType,
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Open(#[from] std::io::Error),
    #[error("Returning a signature from Lab is not yet supported; Transaction can be found and submitted in lab")]
    ReturningSignatureFromLab,
    #[error(transparent)]
    SecureStore(#[from] secure_store::Error),
    #[error(transparent)]
    Ledger(#[from] ledger::Error),
    #[error(transparent)]
    Decode(#[from] stellar_strkey::DecodeError),
}

/// Sign all SorobanAuthorizationEntry's in the transaction with the given signers. Returns a new
/// transaction with the signatures added to each SorobanAuthorizationEntry.
///
/// If no SorobanAuthorizationEntry's need signing (including if none exist), return Ok(None).
///
/// If a SorobanAuthorizationEntry needs signing, but a signature cannot be produced for it,
/// return an Error
pub fn sign_soroban_authorizations(
    raw: &Transaction,
    signers: &[Signer],
    signature_expiration_ledger: u32,
    network_passphrase: &str,
) -> Result<Option<Transaction>, Error> {
    // Check if we have exactly one operation and it's InvokeHostFunction
    let [op @ Operation {
        body: OperationBody::InvokeHostFunction(body),
        ..
    }] = raw.operations.as_slice()
    else {
        return Ok(None);
    };

    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());
    let source_bytes = muxed_account_bytes(&raw.source_account);

    let mut auths_modified = false;
    let mut signed_auths = Vec::with_capacity(body.auth.len());
    for raw_auth in body.auth.as_slice() {
        let mut auth = raw_auth.clone();
        let SorobanAuthorizationEntry {
            credentials: SorobanCredentials::Address(ref mut credentials),
            ..
        } = auth
        else {
            // Doesn't need special signing
            signed_auths.push(auth);
            continue;
        };
        let SorobanAddressCredentials { ref address, .. } = credentials;

        // Before we attempt to sign, validate the auth entry is strict
        match validation::classify_auth_invocation(&body.host_function, &auth.root_invocation) {
            validation::AuthStyle::Strict => {}
            validation::AuthStyle::NonStrict => {
                return Err(Error::NotStrictAuthEntry {
                    auth_entry_str: format_auth_entry(&auth),
                });
            }
            validation::AuthStyle::Invalid => {
                return Err(Error::InvalidAuthEntry {
                    reason: "authorization entry is not expected for the transaction".to_string(),
                    auth_entry_str: format_auth_entry(&auth),
                });
            }
        }

        // See if we have a signer for this authorizationEntry
        // If not, then we Error
        let auth_address_bytes: &[u8; 32] = match address {
            ScAddress::MuxedAccount(_) => todo!("muxed accounts are not supported"),
            ScAddress::ClaimableBalance(_) => todo!("claimable balance not supported"),
            ScAddress::LiquidityPool(_) => todo!("liquidity pool not supported"),
            ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(ref a)))) => a,
            ScAddress::Contract(stellar_xdr::curr::ContractId(Hash(c))) => {
                // This address is for a contract. This means we're using a custom
                // smart-contract account. Currently the CLI doesn't support that yet.
                return Err(Error::MissingSignerForAddress {
                    address: stellar_strkey::Strkey::Contract(stellar_strkey::Contract(*c))
                        .to_string(),
                });
            }
        };

        // Auth entries should not request a signature from the tx source account via the `Address` credential type
        if auth_address_bytes == source_bytes {
            return Err(Error::InvalidAuthEntry {
                reason: "transaction source account is used as credentials".to_string(),
                auth_entry_str: format_auth_entry(&auth),
            });
        }

        let mut signer: Option<&Signer> = None;
        for s in signers {
            if auth_address_bytes == &s.get_public_key()?.0 {
                signer = Some(s);
            }
        }

        match signer {
            Some(signer) => {
                let signed_entry = sign_soroban_authorization_entry(
                    raw_auth,
                    signer,
                    signature_expiration_ledger,
                    &network_id,
                )?;
                signed_auths.push(signed_entry);
                auths_modified = true;
            }
            None => {
                return Err(Error::MissingSignerForAddress {
                    address: stellar_strkey::Strkey::PublicKeyEd25519(
                        stellar_strkey::ed25519::PublicKey(*auth_address_bytes),
                    )
                    .to_string(),
                });
            }
        }
    }

    // If we didn't modify any entries, return Ok(None) to indicate no changes needed to the transaction
    if !auths_modified {
        return Ok(None);
    }

    // Build updated transaction with signed auth entries
    let mut tx = raw.clone();
    let mut new_body = body.clone();
    new_body.auth = signed_auths.try_into()?;
    tx.operations = vec![Operation {
        source_account: op.source_account.clone(),
        body: OperationBody::InvokeHostFunction(new_body),
    }]
    .try_into()?;
    Ok(Some(tx))
}

fn sign_soroban_authorization_entry(
    raw: &SorobanAuthorizationEntry,
    signer: &Signer,
    signature_expiration_ledger: u32,
    network_id: &Hash,
) -> Result<SorobanAuthorizationEntry, Error> {
    let mut auth = raw.clone();
    let SorobanAuthorizationEntry {
        credentials: SorobanCredentials::Address(ref mut credentials),
        ..
    } = auth
    else {
        // Doesn't need special signing
        return Ok(auth);
    };
    let SorobanAddressCredentials { nonce, .. } = credentials;

    let preimage = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
        network_id: network_id.clone(),
        invocation: auth.root_invocation.clone(),
        nonce: *nonce,
        signature_expiration_ledger,
    })
    .to_xdr(Limits::none())?;

    let payload = Sha256::digest(preimage);
    let p: [u8; 32] = payload.as_slice().try_into()?;
    let signature = signer.sign_payload(p)?;
    let public_key_vec = signer.get_public_key()?.0.to_vec();

    let map = ScMap::sorted_from(vec![
        (
            ScVal::Symbol(ScSymbol("public_key".try_into()?)),
            ScVal::Bytes(public_key_vec.try_into().map_err(Error::Xdr)?),
        ),
        (
            ScVal::Symbol(ScSymbol("signature".try_into()?)),
            ScVal::Bytes(
                signature
                    .to_bytes()
                    .to_vec()
                    .try_into()
                    .map_err(Error::Xdr)?,
            ),
        ),
    ])
    .map_err(Error::Xdr)?;
    credentials.signature = ScVal::Vec(Some(
        vec![ScVal::Map(Some(map))].try_into().map_err(Error::Xdr)?,
    ));
    credentials.signature_expiration_ledger = signature_expiration_ledger;
    auth.credentials = SorobanCredentials::Address(credentials.clone());
    Ok(auth)
}

pub struct Signer {
    pub kind: SignerKind,
    pub print: Print,
}

#[allow(clippy::module_name_repetitions, clippy::large_enum_variant)]
pub enum SignerKind {
    Local(LocalKey),
    Ledger(ledger::LedgerType),
    Lab,
    SecureStore(SecureStoreEntry),
}

// It is advised to use the sign_with module, which handles creating a Signer with the appropriate SignerKind
impl Signer {
    pub async fn sign_tx(
        &self,
        tx: Transaction,
        network: &Network,
    ) -> Result<TransactionEnvelope, Error> {
        let tx_env = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx,
            signatures: VecM::default(),
        });
        self.sign_tx_env(&tx_env, network).await
    }

    pub async fn sign_tx_env(
        &self,
        tx_env: &TransactionEnvelope,
        network: &Network,
    ) -> Result<TransactionEnvelope, Error> {
        match &tx_env {
            TransactionEnvelope::Tx(TransactionV1Envelope { tx, signatures }) => {
                let tx_hash = transaction_hash(tx, &network.network_passphrase)?;
                self.print
                    .infoln(format!("Signing transaction: {}", hex::encode(tx_hash)));
                let decorated_signature = self.sign_tx_hash(tx_hash, tx_env, network).await?;
                let mut sigs = signatures.clone().into_vec();
                sigs.push(decorated_signature);
                Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
                    tx: tx.clone(),
                    signatures: sigs.try_into()?,
                }))
            }
            TransactionEnvelope::TxFeeBump(FeeBumpTransactionEnvelope { tx, signatures }) => {
                let tx_hash = fee_bump_transaction_hash(tx, &network.network_passphrase)?;
                self.print.infoln(format!(
                    "Signing fee bump transaction: {}",
                    hex::encode(tx_hash),
                ));
                let decorated_signature = self.sign_tx_hash(tx_hash, tx_env, network).await?;
                let mut sigs = signatures.clone().into_vec();
                sigs.push(decorated_signature);
                Ok(TransactionEnvelope::TxFeeBump(FeeBumpTransactionEnvelope {
                    tx: tx.clone(),
                    signatures: sigs.try_into()?,
                }))
            }
            TransactionEnvelope::TxV0(_) => Err(Error::UnsupportedTransactionEnvelopeType),
        }
    }

    // when we implement this for ledger we'll need it to be async so we can await for the ledger's public key
    pub fn get_public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        match &self.kind {
            SignerKind::Local(local_key) => Ok(stellar_strkey::ed25519::PublicKey::from_payload(
                local_key.key.verifying_key().as_bytes(),
            )?),
            SignerKind::Ledger(_ledger) => todo!("ledger device is not implemented"),
            SignerKind::Lab => Err(Error::ReturningSignatureFromLab),
            SignerKind::SecureStore(secure_store_entry) => secure_store_entry.get_public_key(),
        }
    }

    // when we implement this for ledger we'll need it to be async so we can await the user approved the tx on the ledger device
    pub fn sign_payload(&self, payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
        match &self.kind {
            SignerKind::Local(local_key) => local_key.sign_payload(payload),
            SignerKind::Ledger(_ledger) => todo!("ledger device is not implemented"),
            SignerKind::Lab => Err(Error::ReturningSignatureFromLab),
            SignerKind::SecureStore(secure_store_entry) => secure_store_entry.sign_payload(payload),
        }
    }

    async fn sign_tx_hash(
        &self,
        tx_hash: [u8; 32],
        tx_env: &TransactionEnvelope,
        network: &Network,
    ) -> Result<DecoratedSignature, Error> {
        match &self.kind {
            SignerKind::Local(key) => key.sign_tx_hash(tx_hash),
            SignerKind::Lab => Lab::sign_tx_env(tx_env, network, &self.print),
            SignerKind::Ledger(ledger) => ledger
                .sign_transaction_hash(&tx_hash)
                .await
                .map_err(Error::from),
            SignerKind::SecureStore(entry) => entry.sign_tx_hash(tx_hash),
        }
    }
}

pub struct LocalKey {
    pub key: ed25519_dalek::SigningKey,
}

impl LocalKey {
    pub fn sign_tx_hash(&self, tx_hash: [u8; 32]) -> Result<DecoratedSignature, Error> {
        let hint = SignatureHint(self.key.verifying_key().to_bytes()[28..].try_into()?);
        let signature = Signature(self.key.sign(&tx_hash).to_bytes().to_vec().try_into()?);
        Ok(DecoratedSignature { hint, signature })
    }

    pub fn sign_payload(&self, payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
        Ok(self.key.sign(&payload))
    }
}

pub struct Lab;

impl Lab {
    const URL: &str = "https://lab.stellar.org/transaction/cli-sign";

    pub fn sign_tx_env(
        tx_env: &TransactionEnvelope,
        network: &Network,
        printer: &Print,
    ) -> Result<DecoratedSignature, Error> {
        let xdr = tx_env.to_xdr_base64(Limits::none())?;

        let mut url = url::Url::parse(Self::URL)?;
        url.query_pairs_mut()
            .append_pair("networkPassphrase", &network.network_passphrase)
            .append_pair("xdr", &xdr);
        let url = url.to_string();

        printer.globeln(format!("Opening lab to sign transaction: {url}"));
        open::that(url)?;

        Err(Error::ReturningSignatureFromLab)
    }
}

pub struct SecureStoreEntry {
    pub name: String,
    pub hd_path: Option<usize>,
}

impl SecureStoreEntry {
    pub fn get_public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        Ok(secure_store::get_public_key(&self.name, self.hd_path)?)
    }

    pub fn sign_tx_hash(&self, tx_hash: [u8; 32]) -> Result<DecoratedSignature, Error> {
        let hint = SignatureHint(
            secure_store::get_public_key(&self.name, self.hd_path)?.0[28..].try_into()?,
        );

        let signed_tx_hash = secure_store::sign_tx_data(&self.name, self.hd_path, &tx_hash)?;

        let signature = Signature(signed_tx_hash.clone().try_into()?);
        Ok(DecoratedSignature { hint, signature })
    }

    pub fn sign_payload(&self, payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
        let signed_bytes = secure_store::sign_tx_data(&self.name, self.hd_path, &payload)?;
        let sig = Ed25519Signature::from_bytes(signed_bytes.as_slice().try_into()?);
        Ok(sig)
    }
}

/// Extract the Ed25519 public key bytes from a MuxedAccount
fn muxed_account_bytes(source: &MuxedAccount) -> &[u8; 32] {
    match source {
        MuxedAccount::Ed25519(Uint256(bytes)) => bytes,
        MuxedAccount::MuxedEd25519(muxed) => &muxed.ed25519.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        BytesM, HostFunction, InvokeContractArgs, InvokeHostFunctionOp, Memo, Preconditions,
        SequenceNumber, SorobanAuthorizedFunction, SorobanAuthorizedInvocation, TransactionExt,
    };

    const NETWORK: &str = "Test SDF Network ; September 2015";
    const EXPIRATION_LEDGER: u32 = 100;

    fn local_signer(seed: [u8; 32]) -> Signer {
        Signer {
            kind: SignerKind::Local(LocalKey {
                key: ed25519_dalek::SigningKey::from_bytes(&seed),
            }),
            print: Print::new(true),
        }
    }

    fn signer_pubkey(signer: &Signer) -> [u8; 32] {
        signer.get_public_key().unwrap().0
    }

    fn ed25519_address(bytes: [u8; 32]) -> ScAddress {
        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(bytes))))
    }

    fn invoke_args(contract: [u8; 32], fn_name: &str) -> InvokeContractArgs {
        InvokeContractArgs {
            contract_address: ScAddress::Contract(stellar_xdr::curr::ContractId(Hash(contract))),
            function_name: ScSymbol(fn_name.try_into().unwrap()),
            args: VecM::default(),
        }
    }

    fn invocation(contract: [u8; 32], fn_name: &str) -> SorobanAuthorizedInvocation {
        SorobanAuthorizedInvocation {
            function: SorobanAuthorizedFunction::ContractFn(invoke_args(contract, fn_name)),
            sub_invocations: VecM::default(),
        }
    }

    fn address_auth(
        address: ScAddress,
        invocation: SorobanAuthorizedInvocation,
    ) -> SorobanAuthorizationEntry {
        SorobanAuthorizationEntry {
            credentials: SorobanCredentials::Address(SorobanAddressCredentials {
                address,
                nonce: 0,
                signature_expiration_ledger: 0,
                signature: ScVal::Void,
            }),
            root_invocation: invocation,
        }
    }

    fn build_tx(
        source: MuxedAccount,
        host_function: HostFunction,
        auth: Vec<SorobanAuthorizationEntry>,
    ) -> Transaction {
        Transaction {
            source_account: source,
            fee: 100,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function,
                    auth: auth.try_into().unwrap(),
                }),
            }]
            .try_into()
            .unwrap(),
            ext: TransactionExt::V0,
        }
    }

    /// Pull the embedded public_key bytes out of a signed Address-cred entry.
    fn extract_signed_pubkey(creds: &SorobanAddressCredentials) -> [u8; 32] {
        let ScVal::Vec(Some(outer)) = &creds.signature else {
            panic!("expected ScVal::Vec signature");
        };
        let Some(ScVal::Map(Some(map))) = outer.first() else {
            panic!("expected ScVal::Map inside signature vec");
        };
        map.iter()
            .find_map(|e| match (&e.key, &e.val) {
                (ScVal::Symbol(s), ScVal::Bytes(b)) if s.0.as_slice() == b"public_key" => {
                    Some(b.as_slice().try_into().unwrap())
                }
                _ => None,
            })
            .expect("public_key entry")
    }

    #[test]
    fn test_signs_address_auth_entry_with_matching_signer() {
        let signer = local_signer([1u8; 32]);
        let signer_unused = local_signer([2u8; 32]);
        let signer_pk = signer_pubkey(&signer);
        let source = MuxedAccount::Ed25519(Uint256([9u8; 32]));
        let contract = [42u8; 32];

        let entry = address_auth(ed25519_address(signer_pk), invocation(contract, "hello"));
        let host_fn = HostFunction::InvokeContract(invoke_args(contract, "hello"));
        let tx = build_tx(source, host_fn, vec![entry]);

        let signed_auth_tx =
            sign_soroban_authorizations(&tx, &[signer_unused, signer], EXPIRATION_LEDGER, NETWORK)
                .unwrap()
                .expect("signing modifies the transaction");

        let OperationBody::InvokeHostFunction(body) = &signed_auth_tx.operations[0].body else {
            panic!("expected InvokeHostFunction");
        };
        let SorobanCredentials::Address(creds) = &body.auth[0].credentials else {
            panic!("expected Address credentials");
        };
        assert!(
            !matches!(creds.signature, ScVal::Void),
            "signature should be filled in"
        );
        assert_eq!(creds.signature_expiration_ledger, EXPIRATION_LEDGER);
        assert_eq!(
            extract_signed_pubkey(creds),
            signer_pk,
            "embedded public_key should match the signer"
        );
    }

    #[test]
    fn test_non_strict_auth_returns_error() {
        let signer = local_signer([1u8; 32]);
        let signer_pk = signer_pubkey(&signer);
        let source = MuxedAccount::Ed25519(Uint256([9u8; 32]));
        let contract = [42u8; 32];
        let other_contract = [99u8; 32];

        let entry = address_auth(
            ed25519_address(signer_pk),
            invocation(other_contract, "hello"),
        );
        let host_fn = HostFunction::InvokeContract(invoke_args(contract, "hello"));
        let tx = build_tx(source, host_fn, vec![entry]);

        let result = sign_soroban_authorizations(&tx, &[signer], EXPIRATION_LEDGER, NETWORK);
        assert!(matches!(result, Err(Error::NotStrictAuthEntry { .. })));
    }

    #[test]
    fn test_multiple_entries_with_non_strict_returns_error() {
        let signer = local_signer([1u8; 32]);
        let signer_pk = signer_pubkey(&signer);
        let source = MuxedAccount::Ed25519(Uint256([9u8; 32]));
        let contract = [42u8; 32];
        let other_contract = [99u8; 32];

        let entry = address_auth(ed25519_address(signer_pk), invocation(contract, "hello"));
        let entry_non_strict = address_auth(
            ed25519_address(signer_pk),
            invocation(other_contract, "hello"),
        );
        let host_fn = HostFunction::InvokeContract(invoke_args(contract, "hello"));
        let tx = build_tx(source, host_fn, vec![entry, entry_non_strict]);

        let result = sign_soroban_authorizations(&tx, &[signer], EXPIRATION_LEDGER, NETWORK);
        assert!(matches!(result, Err(Error::NotStrictAuthEntry { .. })));
    }

    #[test]
    fn test_upload_wasm_with_auth_returns_invalid() {
        let signer = local_signer([1u8; 32]);
        let signer_pk = signer_pubkey(&signer);
        let source = MuxedAccount::Ed25519(Uint256([9u8; 32]));
        let wasm: BytesM = [0u8; 32].try_into().unwrap();

        let entry = address_auth(ed25519_address(signer_pk), invocation([42u8; 32], "hello"));
        let host_fn = HostFunction::UploadContractWasm(wasm);
        let tx = build_tx(source, host_fn, vec![entry]);

        let result = sign_soroban_authorizations(&tx, &[signer], EXPIRATION_LEDGER, NETWORK);
        assert!(matches!(result, Err(Error::InvalidAuthEntry { .. })));
    }

    #[test]
    fn test_source_account_as_address_returns_invalid() {
        let signer = local_signer([1u8; 32]);
        let signer_pk = signer_pubkey(&signer);
        let source = MuxedAccount::Ed25519(Uint256(signer_pk));
        let contract = [42u8; 32];

        let entry = address_auth(ed25519_address(signer_pk), invocation(contract, "hello"));
        let host_fn = HostFunction::InvokeContract(invoke_args(contract, "hello"));
        let tx = build_tx(source, host_fn, vec![entry]);

        let result = sign_soroban_authorizations(&tx, &[signer], EXPIRATION_LEDGER, NETWORK);
        assert!(matches!(result, Err(Error::InvalidAuthEntry { .. })));
    }

    #[test]
    fn test_missing_signer_returns_error() {
        let source = MuxedAccount::Ed25519(Uint256([9u8; 32]));
        let contract = [42u8; 32];
        let unknown = [77u8; 32];

        let entry = address_auth(ed25519_address(unknown), invocation(contract, "hello"));
        let host_fn = HostFunction::InvokeContract(invoke_args(contract, "hello"));
        let tx = build_tx(source, host_fn, vec![entry]);

        let result = sign_soroban_authorizations(&tx, &[], EXPIRATION_LEDGER, NETWORK);
        assert!(matches!(result, Err(Error::MissingSignerForAddress { .. })));
    }
}
