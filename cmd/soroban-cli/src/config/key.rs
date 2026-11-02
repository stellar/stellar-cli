use std::{collections::BTreeMap, fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

use super::secret::{self, Secret};
use crate::xdr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to extract secret from public key ")]
    SecretPublicKey,
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),
    #[error("failed to parse key")]
    Parse,
    #[error("expected a public key (G...) or muxed account (M...)")]
    PublicKeyExpected,
    #[error("smart account {0} is a contract, not an account; it can't be a transaction source or sign one")]
    SmartAccountNotAnAccount(stellar_strkey::Contract),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Key {
    #[serde(rename = "public_key")]
    PublicKey(Public),
    #[serde(rename = "muxed_account")]
    MuxedAccount(MuxedAccount),
    #[serde(rename = "smart_account")]
    SmartAccount(SmartAccount),
    #[serde(untagged)]
    Secret(Secret),
}

impl Key {
    pub fn muxed_account(&self, hd_path: Option<u32>) -> Result<xdr::MuxedAccount, Error> {
        let bytes = match self {
            Key::Secret(secret) => secret.public_key(hd_path)?.0,
            Key::PublicKey(Public(key)) => key.0,
            Key::MuxedAccount(MuxedAccount(stellar_strkey::ed25519::MuxedAccount {
                ed25519,
                id,
            })) => {
                return Ok(xdr::MuxedAccount::MuxedEd25519(xdr::MuxedAccountMed25519 {
                    ed25519: xdr::Uint256(*ed25519),
                    id: *id,
                }))
            }
            Key::SmartAccount(account) => {
                return Err(Error::SmartAccountNotAnAccount(account.address.0));
            }
        };
        Ok(xdr::MuxedAccount::Ed25519(xdr::Uint256(bytes)))
    }

    pub fn private_key(
        &self,
        hd_path: Option<u32>,
    ) -> Result<stellar_strkey::ed25519::PrivateKey, Error> {
        match self {
            Key::Secret(secret) => Ok(secret.private_key(hd_path)?),
            _ => Err(Error::SecretPublicKey),
        }
    }

    pub fn parse_public_only(s: &str) -> Result<Self, Error> {
        match s.parse()? {
            key @ (Key::PublicKey(_) | Key::MuxedAccount(_)) => Ok(key),
            Key::Secret(_) | Key::SmartAccount(_) => Err(Error::PublicKeyExpected),
        }
    }
}

impl FromStr for Key {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(secret) = s.parse() {
            return Ok(Key::Secret(secret));
        }
        if let Ok(public_key) = s.parse() {
            return Ok(Key::PublicKey(public_key));
        }
        if let Ok(muxed_account) = s.parse() {
            return Ok(Key::MuxedAccount(muxed_account));
        }
        Err(Error::Parse)
    }
}

/// A contract account whose auth entries are signed by the external
/// `stellar-signer-<signer_plugin>` binary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SmartAccount {
    pub address: Contract,
    pub signer_plugin: String,
    /// Passed through to the plugin as `args`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugin_args: BTreeMap<String, String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde_with::SerializeDisplay, serde_with::DeserializeFromStr,
)]
pub struct Contract(pub stellar_strkey::Contract);

impl FromStr for Contract {
    type Err = stellar_strkey::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Contract(stellar_strkey::Contract::from_string(s)?))
    }
}

impl Display for Contract {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<stellar_strkey::ed25519::PublicKey> for Key {
    fn from(value: stellar_strkey::ed25519::PublicKey) -> Self {
        Key::PublicKey(Public(value))
    }
}

impl From<&stellar_strkey::ed25519::PublicKey> for Key {
    fn from(stellar_strkey::ed25519::PublicKey(key): &stellar_strkey::ed25519::PublicKey) -> Self {
        stellar_strkey::ed25519::PublicKey(*key).into()
    }
}

#[derive(Debug, PartialEq, Eq, serde_with::SerializeDisplay, serde_with::DeserializeFromStr)]
pub struct Public(pub stellar_strkey::ed25519::PublicKey);

impl FromStr for Public {
    type Err = stellar_strkey::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Public(stellar_strkey::ed25519::PublicKey::from_str(s)?))
    }
}

impl Display for Public {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&Public> for stellar_strkey::ed25519::MuxedAccount {
    fn from(Public(stellar_strkey::ed25519::PublicKey(key)): &Public) -> Self {
        stellar_strkey::ed25519::MuxedAccount {
            id: 0,
            ed25519: *key,
        }
    }
}

#[derive(Debug, PartialEq, Eq, serde_with::SerializeDisplay, serde_with::DeserializeFromStr)]
pub struct MuxedAccount(pub stellar_strkey::ed25519::MuxedAccount);

impl FromStr for MuxedAccount {
    type Err = stellar_strkey::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(MuxedAccount(
            stellar_strkey::ed25519::MuxedAccount::from_str(s)?,
        ))
    }
}

impl Display for MuxedAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<Key> for Secret {
    type Error = Error;

    fn try_from(key: Key) -> Result<Secret, Self::Error> {
        match key {
            Key::Secret(secret) => Ok(secret),
            _ => Err(Error::SecretPublicKey),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn round_trip(key: &Key) {
        let serialized = toml::to_string(&key).unwrap();
        println!("{serialized}");
        let deserialized: Key = toml::from_str(&serialized).unwrap();
        assert_eq!(key, &deserialized);
    }

    #[test]
    fn public_key() {
        let key = Key::PublicKey(Public(stellar_strkey::ed25519::PublicKey([0; 32])));
        round_trip(&key);
    }
    #[test]
    fn muxed_key() {
        let key: stellar_strkey::ed25519::MuxedAccount =
            "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU"
                .parse()
                .unwrap();
        let key = Key::MuxedAccount(MuxedAccount(key));
        round_trip(&key);
    }
    #[test]
    fn secret_key() {
        let secret_key = format!(
            "{}",
            stellar_strkey::ed25519::PrivateKey([0; 32]).as_unredacted()
        );
        let secret = Secret::SecretKey { secret_key };
        let key = Key::Secret(secret);
        round_trip(&key);
    }
    #[test]
    fn smart_account() {
        let key = Key::SmartAccount(SmartAccount {
            address: Contract(stellar_strkey::Contract([7; 32])),
            signer_plugin: "oz".to_string(),
            plugin_args: BTreeMap::from([("rule-id".to_string(), "1".to_string())]),
        });
        round_trip(&key);
        assert!(matches!(
            key.muxed_account(None),
            Err(Error::SmartAccountNotAnAccount(_))
        ));
    }
    #[test]
    fn secret_seed_phrase() {
        let seed_phrase = "singer swing mango apple singer swing mango apple singer swing mango apple singer swing mango apple".to_string();
        let secret = Secret::SeedPhrase {
            seed_phrase,
            hd_path: None,
        };
        let key = Key::Secret(secret);
        round_trip(&key);
    }

    const PUBLIC_KEY: &str = "GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC";
    const MUXED_ACCOUNT: &str =
        "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    const SECRET_KEY: &str = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH";
    const SEED_PHRASE: &str =
        "depth decade power loud smile spatial sign movie judge february rate broccoli";

    #[test]
    fn parse_public_only_accepts_public_key() {
        let key = Key::parse_public_only(PUBLIC_KEY).unwrap();
        assert!(matches!(key, Key::PublicKey(_)));
    }

    #[test]
    fn parse_public_only_accepts_muxed_account() {
        let key = Key::parse_public_only(MUXED_ACCOUNT).unwrap();
        assert!(matches!(key, Key::MuxedAccount(_)));
    }

    #[test]
    fn parse_public_only_rejects_secret_key() {
        let err = Key::parse_public_only(SECRET_KEY).unwrap_err();
        assert!(matches!(err, Error::PublicKeyExpected));
    }

    #[test]
    fn parse_public_only_rejects_seed_phrase() {
        let err = Key::parse_public_only(SEED_PHRASE).unwrap_err();
        assert!(matches!(err, Error::PublicKeyExpected));
    }

    #[test]
    fn parse_public_only_rejects_ledger() {
        let err = Key::parse_public_only("ledger").unwrap_err();
        assert!(matches!(err, Error::Parse));
    }

    #[test]
    fn parse_public_only_rejects_secure_store() {
        let err = Key::parse_public_only("secure_store:org.stellar.cli-alice").unwrap_err();
        assert!(matches!(err, Error::PublicKeyExpected));
    }

    #[test]
    fn parse_public_only_rejects_garbage() {
        let err = Key::parse_public_only("not-a-key").unwrap_err();
        assert!(matches!(err, Error::Parse));
    }
}
