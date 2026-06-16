use phf::phf_map;
use sha2::{Digest, Sha256};
use stellar_strkey::ed25519::PrivateKey;

use crate::{
    print::Print,
    xdr::{
        self, Asset, ContractIdPreimage, Hash, HashIdPreimage, HashIdPreimageContractId, Limits,
        ScMap, ScMapEntry, ScVal, Transaction, TransactionEnvelope, TransactionSignaturePayload,
        TransactionSignaturePayloadTaggedTransaction, WriteXdr,
    },
};

pub use soroban_spec_tools::contract as contract_spec;

use crate::config::network::Network;

/// # Errors
///
/// Might return an error
pub fn contract_hash(contract: &[u8]) -> Result<Hash, xdr::Error> {
    Ok(Hash(Sha256::digest(contract).into()))
}

/// Compute the transaction hash for a given transaction envelope.
///
/// # Errors
///
/// If the transaction envelope contains unsupported types (e.g., TxV0), this function will return an error.
/// If an XDR error is encountered during processing, it will be propagated.
pub fn transaction_env_hash(
    tx_env: &TransactionEnvelope,
    network_passphrase: &str,
) -> Result<[u8; 32], xdr::Error> {
    match tx_env {
        TransactionEnvelope::Tx(ref v1_env) => transaction_hash(&v1_env.tx, network_passphrase),
        TransactionEnvelope::TxFeeBump(ref fee_bump_env) => {
            fee_bump_transaction_hash(&fee_bump_env.tx, network_passphrase)
        }
        TransactionEnvelope::TxV0(_) => Err(xdr::Error::Unsupported),
    }
}

/// # Errors
///
/// Might return an error
pub fn transaction_hash(
    tx: &Transaction,
    network_passphrase: &str,
) -> Result<[u8; 32], xdr::Error> {
    let signature_payload = TransactionSignaturePayload {
        network_id: Hash(Sha256::digest(network_passphrase).into()),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone()),
    };
    Ok(Sha256::digest(signature_payload.to_xdr(Limits::none())?).into())
}

/// # Errors
///
/// Might return an error
pub fn fee_bump_transaction_hash(
    fee_bump_tx: &xdr::FeeBumpTransaction,
    network_passphrase: &str,
) -> Result<[u8; 32], xdr::Error> {
    let signature_payload = TransactionSignaturePayload {
        network_id: Hash(Sha256::digest(network_passphrase).into()),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::TxFeeBump(
            fee_bump_tx.clone(),
        ),
    };
    Ok(Sha256::digest(signature_payload.to_xdr(Limits::none())?).into())
}

static EXPLORERS: phf::Map<&'static str, &'static str> = phf_map! {
    "Test SDF Network ; September 2015" => "https://stellar.expert/explorer/testnet",
    "Public Global Stellar Network ; September 2015" => "https://stellar.expert/explorer/public",
};

static LAB_CONTRACT_URLS: phf::Map<&'static str, &'static str> = phf_map! {
    "Test SDF Network ; September 2015" => "https://lab.stellar.org/r/testnet/contract/{contract_id}",
    "Public Global Stellar Network ; September 2015" => "https://lab.stellar.org/r/mainnet/contract/{contract_id}",
};

pub fn explorer_url_for_transaction(network: &Network, tx_hash: &str) -> Option<String> {
    EXPLORERS
        .get(&network.network_passphrase)
        .map(|base_url| format!("{base_url}/tx/{tx_hash}"))
}

pub fn lab_url_for_contract(
    network: &Network,
    contract_id: &stellar_strkey::Contract,
) -> Option<String> {
    LAB_CONTRACT_URLS
        .get(&network.network_passphrase)
        .map(|base_url| base_url.replace("{contract_id}", &contract_id.to_string()))
}

/// # Errors
///
/// Might return an error
pub fn contract_id_from_str(
    contract_id: &str,
) -> Result<stellar_strkey::Contract, stellar_strkey::DecodeError> {
    Ok(
        if let Ok(strkey) = stellar_strkey::Contract::from_string(contract_id) {
            strkey
        } else {
            // strkey failed, try to parse it as a hex string, for backwards compatibility.
            stellar_strkey::Contract(
                soroban_spec_tools::utils::padded_hex_from_str(contract_id, 32)
                    .map_err(|_| stellar_strkey::DecodeError::Invalid)?
                    .try_into()
                    .map_err(|_| stellar_strkey::DecodeError::Invalid)?,
            )
        },
    )
}

/// # Errors
/// May not find a config dir
pub fn find_config_dir(mut pwd: std::path::PathBuf) -> std::io::Result<std::path::PathBuf> {
    loop {
        let stellar_dir = pwd.join(".stellar");
        let stellar_exists = stellar_dir.exists();

        let soroban_dir = pwd.join(".soroban");
        let soroban_exists = soroban_dir.exists();

        if stellar_exists && soroban_exists {
            tracing::warn!("the .stellar and .soroban config directories exist at path {pwd:?}, using the .stellar");
        }

        if stellar_exists {
            return Ok(stellar_dir);
        }

        if soroban_exists {
            return Ok(soroban_dir);
        }

        if !pwd.pop() {
            break;
        }
    }

    Err(std::io::Error::other("stellar directory not found"))
}

pub(crate) fn into_signing_key(key: &PrivateKey) -> ed25519_dalek::SigningKey {
    let secret: ed25519_dalek::SecretKey = key.0;
    ed25519_dalek::SigningKey::from_bytes(&secret)
}

pub fn deprecate_message(print: Print, arg: &str, hint: &str) {
    print.warnln(
        format!("`{arg}` is deprecated and will be removed in future versions of the CLI. {hint}")
            .trim(),
    );
}

/// Used in tests
#[allow(unused)]
pub(crate) fn parse_secret_key(
    s: &str,
) -> Result<ed25519_dalek::SigningKey, stellar_strkey::DecodeError> {
    Ok(into_signing_key(&PrivateKey::from_string(s)?))
}

pub fn is_hex_string(s: &str) -> bool {
    s.chars().all(|s| s.is_ascii_hexdigit())
}

pub fn escape_control_characters(s: &str) -> String {
    use std::fmt::Write as _;
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_control() {
            let mut buf = [0u8; 4];
            for &byte in c.encode_utf8(&mut buf).as_bytes() {
                write!(result, "\\x{byte:02x}").unwrap();
            }
        } else {
            result.push(c);
        }
    }
    result
}

pub fn contract_id_hash_from_asset(
    asset: &Asset,
    network_passphrase: &str,
) -> stellar_strkey::Contract {
    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());
    let preimage = HashIdPreimage::ContractId(HashIdPreimageContractId {
        network_id,
        contract_id_preimage: ContractIdPreimage::Asset(asset.clone()),
    });
    let preimage_xdr = preimage
        .to_xdr(Limits::none())
        .expect("HashIdPreimage should not fail encoding to xdr");
    stellar_strkey::Contract(Sha256::digest(preimage_xdr).into())
}

pub fn get_name_from_stellar_asset_contract_storage(storage: &ScMap) -> Option<String> {
    if let Some(ScMapEntry {
        val: ScVal::Map(Some(map)),
        ..
    }) = storage
        .iter()
        .find(|ScMapEntry { key, .. }| key == &ScVal::Symbol("METADATA".try_into().unwrap()))
    {
        if let Some(ScMapEntry {
            val: ScVal::String(name),
            ..
        }) = map
            .iter()
            .find(|ScMapEntry { key, .. }| key == &ScVal::Symbol("name".try_into().unwrap()))
        {
            Some(name.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

pub mod http {
    use std::time::Duration;

    use crate::commands::version;
    fn user_agent() -> String {
        format!("{}/{}", env!("CARGO_PKG_NAME"), version::pkg())
    }

    const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

    /// Creates and returns a configured `reqwest::Client`.
    ///
    /// # Panics
    ///
    /// Panics if the Client initialization fails.
    pub fn client() -> reqwest::Client {
        // Why we panic here:
        // 1. Client initialization failures are rare and usually indicate serious issues.
        // 2. The application cannot function properly without a working HTTP client.
        // 3. This simplifies error handling for callers, as they can assume a valid client.
        reqwest::Client::builder()
            .user_agent(user_agent())
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("Failed to build reqwest client")
    }

    /// Creates and returns a configured `reqwest::blocking::Client`.
    ///
    /// # Panics
    ///
    /// Panics if the Client initialization fails.
    pub fn blocking_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .user_agent(user_agent())
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("Failed to build reqwest blocking client")
    }
}

pub mod url {
    use url::Url;

    /// Returns the given URL with any password component replaced by the literal
    /// `redacted`. If the URL is not parseable, it is returned unchanged.
    pub fn redact_url(url: &str) -> String {
        let Ok(mut url) = Url::parse(url) else {
            return url.to_string();
        };
        if url.password().is_some() {
            let _ = url.set_password(Some("redacted"));
        }
        url.to_string()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn leaves_url_without_password_unchanged() {
            let plain = "https://rpc.example.com/soroban";
            assert_eq!(redact_url(plain), plain);

            let user_only = "https://alice@rpc.example.com/soroban";
            assert_eq!(redact_url(user_only), user_only);
        }

        #[test]
        fn replaces_password_with_placeholder() {
            let with_password = "https://alice:supersecret@rpc.example.com/soroban";
            let redacted = redact_url(with_password);
            assert!(
                !redacted.contains("supersecret"),
                "password leaked: {redacted}"
            );
            assert!(
                redacted.contains("alice:redacted"),
                "expected `alice:redacted`: {redacted}"
            );
            assert!(
                redacted.contains("rpc.example.com/soroban"),
                "expected host and path preserved: {redacted}"
            );
        }

        #[test]
        fn returns_input_when_unparseable() {
            let bad = "not a url";
            assert_eq!(redact_url(bad), bad);
        }
    }
}

pub mod args {
    #[derive(thiserror::Error, Debug)]
    pub enum DeprecatedError<'a> {
        #[error("This argument has been removed and will be not be recognized by the future versions of CLI: {0}"
        )]
        RemovedArgument(&'a str),
    }

    #[macro_export]
    /// Mark argument as removed with an error to be printed when it's used.
    macro_rules! error_on_use_of_removed_arg {
        ($_type:ident, $message: expr) => {
            |a: &str| {
                Err::<$_type, utils::args::DeprecatedError>(
                    utils::args::DeprecatedError::RemovedArgument($message),
                )
            }
        };
    }

    /// Mark argument as deprecated with warning to be printed when it's used.
    #[macro_export]
    macro_rules! deprecated_arg {
        (bool, $message: expr) => {
            <_ as clap::builder::TypedValueParser>::map(
                clap::builder::BoolValueParser::new(),
                |x| {
                    if (x) {
                        $crate::print::Print::new(false).warnln($message);
                    }
                    x
                },
            )
        };
    }
}

pub mod rpc {
    use crate::xdr;
    use soroban_rpc::{Client, Error};
    use stellar_xdr::curr::{Hash, LedgerEntryData, LedgerKey, Limits, ReadXdr};

    pub async fn get_remote_wasm_from_hash(client: &Client, hash: &Hash) -> Result<Vec<u8>, Error> {
        let code_key = LedgerKey::ContractCode(xdr::LedgerKeyContractCode { hash: hash.clone() });
        let contract_data = client.get_ledger_entries(&[code_key]).await?;
        let entries = contract_data.entries.unwrap_or_default();
        if entries.is_empty() {
            return Err(Error::NotFound(
                "Contract Code".to_string(),
                hex::encode(hash),
            ));
        }
        let contract_data_entry = &entries[0];
        let code = match LedgerEntryData::from_xdr_base64(&contract_data_entry.xdr, Limits::none())?
        {
            LedgerEntryData::ContractCode(xdr::ContractCodeEntry { code, .. }) => Vec::from(code),
            scval => return Err(Error::UnexpectedContractCodeDataType(scval)),
        };
        super::verify_wasm_hash(&code, hash)?;
        Ok(code)
    }
}

// Uses `Error::NotFound` because `soroban_rpc::Error` has no integrity/mismatch
// variant. The message makes the actual failure reason clear.
fn verify_wasm_hash(code: &[u8], expected_hash: &Hash) -> Result<(), soroban_rpc::Error> {
    let computed_hash = Hash(Sha256::digest(code).into());
    if computed_hash != *expected_hash {
        return Err(soroban_rpc::Error::NotFound(
            "WASM hash mismatch".to_string(),
            format!(
                "expected {}, got {}",
                hex::encode(expected_hash.0),
                hex::encode(computed_hash.0),
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_id_from_str() {
        // strkey
        match contract_id_from_str("CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE") {
            Ok(contract_id) => assert_eq!(
                contract_id.0,
                [
                    0x36, 0x3e, 0xaa, 0x38, 0x67, 0x84, 0x1f, 0xba, 0xd0, 0xf4, 0xed, 0x88, 0xc7,
                    0x79, 0xe4, 0xfe, 0x66, 0xe5, 0x6a, 0x24, 0x70, 0xdc, 0x98, 0xc0, 0xec, 0x9c,
                    0x07, 0x3d, 0x05, 0xc7, 0xb1, 0x03,
                ]
            ),
            Err(err) => panic!("Failed to parse contract id: {err}"),
        }
    }

    #[test]
    fn test_verify_wasm_hash_matching() {
        use sha2::{Digest, Sha256};
        use stellar_xdr::curr::Hash;

        let wasm_bytes = b"\0asm fake wasm content";
        let correct_hash = Hash(Sha256::digest(wasm_bytes).into());
        assert!(verify_wasm_hash(wasm_bytes, &correct_hash).is_ok());
    }

    #[test]
    fn test_verify_wasm_hash_mismatch() {
        use stellar_xdr::curr::Hash;

        let wasm_bytes = b"\0asm fake wasm content";
        let wrong_hash = Hash([0xAB; 32]);
        let err = verify_wasm_hash(wasm_bytes, &wrong_hash).unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("WASM hash mismatch"),
            "expected 'WASM hash mismatch' in error: {err_msg}"
        );
        assert!(
            err_msg.contains("abababababababababababababababababababababababababababababababab"),
            "expected expected-hash in error: {err_msg}"
        );
        assert!(
            err_msg.contains("501dc4e05f47c4713c4a27e89a5b07ed769bb2cc858bcf46de9bed13ae65af29"),
            "expected computed-hash in error: {err_msg}"
        );
    }
}
