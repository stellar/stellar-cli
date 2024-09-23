use ed25519_dalek::ed25519::signature::Signer;
use sha2::{Digest, Sha256};

use soroban_env_host::xdr::{
    self, AccountId, DecoratedSignature, Hash, HashIdPreimage, HashIdPreimageSorobanAuthorization,
    InvokeHostFunctionOp, Limits, Operation, OperationBody, PublicKey, ScAddress, ScMap, ScSymbol,
    ScVal, Signature, SignatureHint, SorobanAddressCredentials, SorobanAuthorizationEntry,
    SorobanAuthorizedFunction, SorobanCredentials, Transaction, TransactionEnvelope,
    TransactionV1Envelope, Uint256, WriteXdr,
};

pub mod types;
use crate::utils::transaction_hash;
pub use types::{LocalKey, SignTx};

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
    #[error("User cancelled signing, perhaps need to add -y")]
    UserCancelledSigning,
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

fn requires_auth(txn: &Transaction) -> Option<xdr::Operation> {
    let [op @ Operation {
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp { auth, .. }),
        ..
    }] = txn.operations.as_slice()
    else {
        return None;
    };
    matches!(
        auth.first().map(|x| &x.root_invocation.function),
        Some(&SorobanAuthorizedFunction::ContractFn(_))
    )
    .then(move || op.clone())
}

// Use the given source_key and signers, to sign all SorobanAuthorizationEntry's in the given
// transaction. If unable to sign, return an error.
pub fn sign_soroban_authorizations(
    raw: &Transaction,
    source_key: &ed25519_dalek::SigningKey,
    signers: &[ed25519_dalek::SigningKey],
    signature_expiration_ledger: u32,
    network_passphrase: &str,
) -> Result<Option<Transaction>, Error> {
    let mut tx = raw.clone();
    let Some(mut op) = requires_auth(&tx) else {
        return Ok(None);
    };

    let Operation {
        body: OperationBody::InvokeHostFunction(ref mut body),
        ..
    } = op
    else {
        return Ok(None);
    };

    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());

    let verification_key = source_key.verifying_key();
    let source_address = verification_key.as_bytes();

    let signed_auths = body
        .auth
        .as_slice()
        .iter()
        .map(|raw_auth| {
            let mut auth = raw_auth.clone();
            let SorobanAuthorizationEntry {
                credentials: SorobanCredentials::Address(ref mut credentials),
                ..
            } = auth
            else {
                // Doesn't need special signing
                return Ok(auth);
            };
            let SorobanAddressCredentials { ref address, .. } = credentials;

            // See if we have a signer for this authorizationEntry
            // If not, then we Error
            let needle = match address {
                ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(ref a)))) => a,
                ScAddress::Contract(Hash(c)) => {
                    // This address is for a contract. This means we're using a custom
                    // smart-contract account. Currently the CLI doesn't support that yet.
                    return Err(Error::MissingSignerForAddress {
                        address: stellar_strkey::Strkey::Contract(stellar_strkey::Contract(*c))
                            .to_string(),
                    });
                }
            };
            let signer = if let Some(s) = signers
                .iter()
                .find(|s| needle == s.verifying_key().as_bytes())
            {
                s
            } else if needle == source_address {
                // This is the source address, so we can sign it
                source_key
            } else {
                // We don't have a signer for this address
                return Err(Error::MissingSignerForAddress {
                    address: stellar_strkey::Strkey::PublicKeyEd25519(
                        stellar_strkey::ed25519::PublicKey(*needle),
                    )
                    .to_string(),
                });
            };

            sign_soroban_authorization_entry(
                raw_auth,
                signer,
                signature_expiration_ledger,
                &network_id,
            )
        })
        .collect::<Result<Vec<_>, Error>>()?;

    body.auth = signed_auths.try_into()?;
    tx.operations = vec![op].try_into()?;
    Ok(Some(tx))
}

fn sign_soroban_authorization_entry(
    raw: &SorobanAuthorizationEntry,
    signer: &ed25519_dalek::SigningKey,
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
    let signature = signer.sign(&payload);

    let map = ScMap::sorted_from(vec![
        (
            ScVal::Symbol(ScSymbol("public_key".try_into()?)),
            ScVal::Bytes(
                signer
                    .verifying_key()
                    .to_bytes()
                    .to_vec()
                    .try_into()
                    .map_err(Error::Xdr)?,
            ),
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

pub fn sign_tx(
    key: &ed25519_dalek::SigningKey,
    tx: &Transaction,
    network_passphrase: &str,
) -> Result<TransactionEnvelope, Error> {
    let tx_hash = transaction_hash(tx, network_passphrase)?;
    let tx_signature = key.sign(&tx_hash);

    let decorated_signature = DecoratedSignature {
        hint: SignatureHint(key.verifying_key().to_bytes()[28..].try_into()?),
        signature: Signature(tx_signature.to_bytes().try_into()?),
    };

    Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: tx.clone(),
        signatures: [decorated_signature].try_into()?,
    }))
}
