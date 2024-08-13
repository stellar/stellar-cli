use sha2::{Digest, Sha256};

use crate::{
    config::network::Network,
    xdr::{
        self, AccountId, Hash, HashIdPreimage, HashIdPreimageSorobanAuthorization, Limits,
        OperationBody, PublicKey, ScAddress, ScMap, ScSymbol, ScVal, SorobanAddressCredentials,
        SorobanAuthorizationEntry, SorobanCredentials, Transaction, Uint256, WriteXdr,
    },
};

use super::{extract_auth_operation, hash, Error, Stellar};

/// Sign a Soroban authorization entries for a given transaction and set the expiration ledger
/// # Errors
/// Returns an error if the address is not found
pub async fn sign_soroban_authorizations(
    signer: &impl Stellar,
    raw: &Transaction,
    network: &Network,
    expiration_ledger: u32,
) -> Result<Option<Transaction>, Error> {
    let mut tx = raw.clone();
    let Some(mut op) = extract_auth_operation(&tx) else {
        return Ok(None);
    };

    let xdr::Operation {
        body: OperationBody::InvokeHostFunction(ref mut body),
        ..
    } = op
    else {
        return Ok(None);
    };
    let mut auths = body.auth.to_vec();
    for auth in &mut auths {
        *auth = maybe_sign_soroban_authorization_entry(signer, auth, network, expiration_ledger)
            .await?;
    }
    body.auth = auths.try_into()?;
    tx.operations = [op].try_into()?;
    Ok(Some(tx))
}

/// Sign a Soroban authorization entry if the address is public key
/// # Errors
/// Returns an error if the address in entry is a contract
pub async fn maybe_sign_soroban_authorization_entry(
    signer: &impl Stellar,
    unsigned_entry: &SorobanAuthorizationEntry,
    network: &Network,
    expiration_ledger: u32,
) -> Result<SorobanAuthorizationEntry, Error> {
    if let SorobanAuthorizationEntry {
        credentials: SorobanCredentials::Address(SorobanAddressCredentials { address, .. }),
        ..
    } = unsigned_entry
    {
        // See if we have a signer for this authorizationEntry
        // If not, then we Error
        let key = match address {
            ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(a)))) => {
                stellar_strkey::ed25519::PublicKey(*a)
            }
            ScAddress::Contract(Hash(c)) => {
                // This address is for a contract. This means we're using a custom
                // smart-contract account. Currently the CLI doesn't support that yet.
                return Err(Error::MissingSignerForAddress {
                    address: stellar_strkey::Strkey::Contract(stellar_strkey::Contract(*c))
                        .to_string(),
                });
            }
        };
        if key == signer.get_public_key().await? {
            return sign_soroban_authorization_entry(
                signer,
                unsigned_entry,
                network,
                expiration_ledger,
            )
            .await;
        }
    }
    Ok(unsigned_entry.clone())
}

/// Sign a Soroban authorization entry with the given address
/// # Errors
/// Returns an error if the address is not found
pub async fn sign_soroban_authorization_entry(
    signer: &impl Stellar,
    unsigned_entry: &SorobanAuthorizationEntry,
    Network {
        network_passphrase, ..
    }: &Network,
    expiration_ledger: u32,
) -> Result<SorobanAuthorizationEntry, Error> {
    let address = signer.get_public_key().await?;
    let mut auth = unsigned_entry.clone();
    let SorobanAuthorizationEntry {
        credentials: SorobanCredentials::Address(ref mut credentials),
        ..
    } = auth
    else {
        // Doesn't need special signing
        return Ok(auth);
    };
    let SorobanAddressCredentials {
        nonce,
        signature_expiration_ledger,
        ..
    } = credentials;

    *signature_expiration_ledger = expiration_ledger;

    let preimage = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
        network_id: hash(network_passphrase),
        invocation: auth.root_invocation.clone(),
        nonce: *nonce,
        signature_expiration_ledger: *signature_expiration_ledger,
    })
    .to_xdr(Limits::none())?;

    let payload = Sha256::digest(preimage);
    let signature = signer.sign_blob(&payload).await?;

    let map = ScMap::sorted_from(vec![
        (
            ScVal::Symbol(ScSymbol("public_key".try_into()?)),
            ScVal::Bytes(address.0.to_vec().try_into()?),
        ),
        (
            ScVal::Symbol(ScSymbol("signature".try_into()?)),
            ScVal::Bytes(signature.try_into()?),
        ),
    ])?;
    credentials.signature = ScVal::Vec(Some(vec![ScVal::Map(Some(map))].try_into()?));
    auth.credentials = SorobanCredentials::Address(credentials.clone());

    Ok(auth)
}
