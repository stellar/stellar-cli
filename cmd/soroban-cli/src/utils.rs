use std::{collections::HashMap, io::ErrorKind, path::Path};

use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};

use soroban_env_host::{
    budget::Budget,
    expiration_ledger_bumps::ExpirationLedgerBumps,
    storage::{AccessType, Footprint, Storage},
    xdr::{
        AccountEntry, AccountEntryExt, AccountId, Asset, ContractCodeEntry, ContractCodeEntryBody,
        ContractDataDurability, ContractDataEntry, ContractDataEntryBody, ContractDataEntryData,
        ContractEntryBodyType, ContractExecutable, ContractIdPreimage, DecoratedSignature,
        Error as XdrError, ExtensionPoint, Hash, HashIdPreimage, HashIdPreimageContractId,
        LedgerEntry, LedgerEntryData, LedgerEntryExt, LedgerFootprint, LedgerKey,
        LedgerKeyContractCode, LedgerKeyContractData, ScAddress, ScContractInstance, ScSpecEntry,
        ScVal, SequenceNumber, Signature, SignatureHint, String32, Thresholds, Transaction,
        TransactionEnvelope, TransactionSignaturePayload,
        TransactionSignaturePayloadTaggedTransaction, TransactionV1Envelope, VecM, WriteXdr,
    },
};
use soroban_ledger_snapshot::LedgerSnapshot;
use soroban_sdk::token;
use soroban_spec::read::FromWasmError;
use stellar_strkey::ed25519::PrivateKey;

use crate::network::sandbox_network_id;

pub mod contract_spec;

/// # Errors
///
/// Might return an error
pub fn contract_hash(contract: &[u8]) -> Result<Hash, XdrError> {
    Ok(Hash(Sha256::digest(contract).into()))
}

/// # Errors
///
/// Might return an error
pub fn ledger_snapshot_read_or_default(
    p: impl AsRef<Path>,
) -> Result<LedgerSnapshot, soroban_ledger_snapshot::Error> {
    match LedgerSnapshot::read_file(p) {
        Ok(snapshot) => Ok(snapshot),
        Err(soroban_ledger_snapshot::Error::Io(e)) if e.kind() == ErrorKind::NotFound => {
            Ok(LedgerSnapshot {
                network_id: sandbox_network_id(),
                // These three "defaults" are not part of the actual default definition in
                // rs-soroban-sdk, but if we don't have them the sandbox doesn't work right.
                // Oof.
                // TODO: Remove this hacky workaround.
                min_persistent_entry_expiration: 4096,
                min_temp_entry_expiration: 16,
                max_entry_expiration: 6_312_000,
                ..Default::default()
            })
        }
        Err(e) => Err(e),
    }
}

/// # Errors
///
/// Might return an error
pub fn add_contract_code_to_ledger_entries(
    entries: &mut Vec<(Box<LedgerKey>, Box<LedgerEntry>)>,
    contract: Vec<u8>,
    min_persistent_entry_expiration: u32,
) -> Result<Hash, XdrError> {
    // Install the code
    let hash = contract_hash(contract.as_slice())?;
    let code_key = LedgerKey::ContractCode(LedgerKeyContractCode {
        hash: hash.clone(),
        body_type: ContractEntryBodyType::DataEntry,
    });
    let code_entry = LedgerEntry {
        last_modified_ledger_seq: 0,
        data: LedgerEntryData::ContractCode(ContractCodeEntry {
            ext: ExtensionPoint::V0,
            hash: hash.clone(),
            body: ContractCodeEntryBody::DataEntry(contract.try_into()?),
            expiration_ledger_seq: min_persistent_entry_expiration,
        }),
        ext: LedgerEntryExt::V0,
    };
    for (k, e) in entries.iter_mut() {
        if **k == code_key {
            **e = code_entry;
            return Ok(hash);
        }
    }
    entries.push((Box::new(code_key), Box::new(code_entry)));
    Ok(hash)
}

pub fn add_contract_to_ledger_entries(
    entries: &mut Vec<(Box<LedgerKey>, Box<LedgerEntry>)>,
    contract_id: [u8; 32],
    wasm_hash: [u8; 32],
    min_persistent_entry_expiration: u32,
) {
    // Create the contract
    let contract_key = LedgerKey::ContractData(LedgerKeyContractData {
        contract: ScAddress::Contract(contract_id.into()),
        key: ScVal::LedgerKeyContractInstance,
        durability: ContractDataDurability::Persistent,
        body_type: ContractEntryBodyType::DataEntry,
    });

    let contract_entry = LedgerEntry {
        last_modified_ledger_seq: 0,
        data: LedgerEntryData::ContractData(ContractDataEntry {
            contract: ScAddress::Contract(contract_id.into()),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
            body: ContractDataEntryBody::DataEntry(ContractDataEntryData {
                flags: 0,
                val: ScVal::ContractInstance(ScContractInstance {
                    executable: ContractExecutable::Wasm(Hash(wasm_hash)),
                    storage: None,
                }),
            }),
            expiration_ledger_seq: min_persistent_entry_expiration,
        }),
        ext: LedgerEntryExt::V0,
    };
    for (k, e) in entries.iter_mut() {
        if **k == contract_key {
            **e = contract_entry;
            return;
        }
    }
    entries.push((Box::new(contract_key), Box::new(contract_entry)));
}

pub fn bump_ledger_entry_expirations(
    entries: &mut [(Box<LedgerKey>, Box<LedgerEntry>)],
    bumps: &ExpirationLedgerBumps,
) {
    // let lookup: HashMap<LedgerKey, u32> = bumps
    let lookup = bumps
        .iter()
        .map(|b| (b.key.as_ref().clone(), b.min_expiration))
        .collect::<HashMap<_, _>>();
    for (k, e) in entries.iter_mut() {
        if let Some(min_expiration) = lookup.get(k.as_ref()) {
            if let LedgerEntryData::ContractData(entry) = &mut e.data {
                entry.expiration_ledger_seq = *min_expiration;
            }
        }
    }
}

/// # Errors
///
/// Might return an error
pub fn transaction_hash(tx: &Transaction, network_passphrase: &str) -> Result<[u8; 32], XdrError> {
    let signature_payload = TransactionSignaturePayload {
        network_id: Hash(Sha256::digest(network_passphrase).into()),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone()),
    };
    Ok(Sha256::digest(signature_payload.to_xdr()?).into())
}

/// # Errors
///
/// Might return an error
pub fn sign_transaction(
    key: &ed25519_dalek::Keypair,
    tx: &Transaction,
    network_passphrase: &str,
) -> Result<TransactionEnvelope, XdrError> {
    let tx_hash = transaction_hash(tx, network_passphrase)?;
    let tx_signature = key.sign(&tx_hash);

    let decorated_signature = DecoratedSignature {
        hint: SignatureHint(key.public.to_bytes()[28..].try_into()?),
        signature: Signature(tx_signature.to_bytes().try_into()?),
    };

    Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: tx.clone(),
        signatures: vec![decorated_signature].try_into()?,
    }))
}

/// # Errors
///
/// Might return an error
pub fn contract_id_from_str(contract_id: &str) -> Result<[u8; 32], stellar_strkey::DecodeError> {
    stellar_strkey::Contract::from_string(contract_id)
        .map(|strkey| strkey.0)
        .or_else(|_| {
            // strkey failed, try to parse it as a hex string, for backwards compatibility.
            soroban_spec_tools::utils::padded_hex_from_str(contract_id, 32)
                .map_err(|_| stellar_strkey::DecodeError::Invalid)?
                .try_into()
                .map_err(|_| stellar_strkey::DecodeError::Invalid)
        })
        .map_err(|_| stellar_strkey::DecodeError::Invalid)
}

/// # Errors
///
/// Might return an error
pub fn get_contract_spec_from_storage(
    storage: &mut Storage,
    current_ledger_seq: &u32,
    contract_id: [u8; 32],
) -> Result<Vec<ScSpecEntry>, FromWasmError> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract: ScAddress::Contract(contract_id.into()),
        key: ScVal::LedgerKeyContractInstance,
        body_type: ContractEntryBodyType::DataEntry,
        durability: ContractDataDurability::Persistent,
    });
    match storage.get(&key.into(), &Budget::default()) {
        Ok(rc) => match rc.as_ref() {
            LedgerEntry {
                data:
                    LedgerEntryData::ContractData(ContractDataEntry {
                        body:
                            ContractDataEntryBody::DataEntry(ContractDataEntryData {
                                val: ScVal::ContractInstance(ScContractInstance { executable, .. }),
                                ..
                            }),
                        expiration_ledger_seq,
                        ..
                    }),
                ..
            } => match executable {
                ContractExecutable::Token => {
                    if expiration_ledger_seq <= current_ledger_seq {
                        return Err(FromWasmError::NotFound);
                    }
                    let res = soroban_spec::read::parse_raw(&token::StellarAssetSpec::spec_xdr());
                    res.map_err(FromWasmError::Parse)
                }
                ContractExecutable::Wasm(hash) => {
                    if expiration_ledger_seq <= current_ledger_seq {
                        return Err(FromWasmError::NotFound);
                    }
                    if let Ok(rc) = storage.get(
                        &LedgerKey::ContractCode(LedgerKeyContractCode {
                            hash: hash.clone(),
                            body_type: ContractEntryBodyType::DataEntry,
                        })
                        .into(),
                        &Budget::default(),
                    ) {
                        match rc.as_ref() {
                            LedgerEntry {
                                data:
                                    LedgerEntryData::ContractCode(ContractCodeEntry {
                                        body: ContractCodeEntryBody::DataEntry(code),
                                        expiration_ledger_seq,
                                        ..
                                    }),
                                ..
                            } => {
                                if expiration_ledger_seq <= current_ledger_seq {
                                    return Err(FromWasmError::NotFound);
                                }
                                soroban_spec::read::from_wasm(code.as_vec())
                            }
                            _ => Err(FromWasmError::NotFound),
                        }
                    } else {
                        Err(FromWasmError::NotFound)
                    }
                }
            },
            _ => Err(FromWasmError::NotFound),
        },
        _ => Err(FromWasmError::NotFound),
    }
}

/// # Panics
///
/// May panic
#[must_use]
pub fn create_ledger_footprint(footprint: &Footprint) -> LedgerFootprint {
    let mut read_only: Vec<LedgerKey> = vec![];
    let mut read_write: Vec<LedgerKey> = vec![];
    let Footprint(m) = footprint;
    for (k, v) in m {
        let dest = match v {
            AccessType::ReadOnly => &mut read_only,
            AccessType::ReadWrite => &mut read_write,
        };
        dest.push((**k).clone());
    }
    LedgerFootprint {
        read_only: read_only.try_into().unwrap(),
        read_write: read_write.try_into().unwrap(),
    }
}

#[must_use]
pub fn default_account_ledger_entry(account_id: AccountId) -> LedgerEntry {
    // TODO: Consider moving the definition of a default account ledger entry to
    // a location shared by the SDK and CLI. The SDK currently defines the same
    // value (see URL below). There's some benefit in only defining this once to
    // prevent the two from diverging, which would cause inconsistent test
    // behavior between the SDK and CLI. A good home for this is unclear at this
    // time.
    // https://github.com/stellar/rs-soroban-sdk/blob/b6f9a2c7ec54d2d5b5a1e02d1e38ae3158c22e78/soroban-sdk/src/accounts.rs#L470-L483.
    LedgerEntry {
        data: LedgerEntryData::Account(AccountEntry {
            account_id,
            balance: 0,
            flags: 0,
            home_domain: String32::default(),
            inflation_dest: None,
            num_sub_entries: 0,
            seq_num: SequenceNumber(0),
            thresholds: Thresholds([1; 4]),
            signers: VecM::default(),
            ext: AccountEntryExt::V0,
        }),
        last_modified_ledger_seq: 0,
        ext: LedgerEntryExt::V0,
    }
}

/// # Errors
/// May not find a config dir
pub fn find_config_dir(mut pwd: std::path::PathBuf) -> std::io::Result<std::path::PathBuf> {
    let soroban_dir = |p: &std::path::Path| p.join(".soroban");
    while !soroban_dir(&pwd).exists() {
        if !pwd.pop() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "soroban directory not found",
            ));
        }
    }
    Ok(soroban_dir(&pwd))
}

pub(crate) fn into_key_pair(
    key: &PrivateKey,
) -> Result<ed25519_dalek::Keypair, ed25519_dalek::SignatureError> {
    let secret = ed25519_dalek::SecretKey::from_bytes(&key.0)?;
    let public = (&secret).into();
    Ok(ed25519_dalek::Keypair { secret, public })
}

/// Used in tests
#[allow(unused)]
pub(crate) fn parse_secret_key(
    s: &str,
) -> Result<ed25519_dalek::Keypair, ed25519_dalek::SignatureError> {
    into_key_pair(&PrivateKey::from_string(s).unwrap())
}

pub fn is_hex_string(s: &str) -> bool {
    s.chars().all(|s| s.is_ascii_hexdigit())
}

pub fn contract_id_hash_from_asset(
    asset: &Asset,
    network_passphrase: &str,
) -> Result<Hash, XdrError> {
    let network_id = Hash(
        Sha256::digest(network_passphrase.as_bytes())
            .try_into()
            .unwrap(),
    );
    let preimage = HashIdPreimage::ContractId(HashIdPreimageContractId {
        network_id,
        contract_id_preimage: ContractIdPreimage::Asset(asset.clone()),
    });
    let preimage_xdr = preimage.to_xdr()?;
    Ok(Hash(Sha256::digest(preimage_xdr).into()))
}

pub mod parsing {

    use regex::Regex;
    use soroban_env_host::xdr::{
        AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, PublicKey,
    };

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("invalid asset code: {asset}")]
        InvalidAssetCode { asset: String },
        #[error("cannot parse account id: {account_id}")]
        CannotParseAccountId { account_id: String },
        #[error("cannot parse asset: {asset}")]
        CannotParseAsset { asset: String },
    }

    pub fn parse_asset(str: &str) -> Result<Asset, Error> {
        if str == "native" {
            return Ok(Asset::Native);
        }
        let split: Vec<&str> = str.splitn(2, ':').collect();
        if split.len() != 2 {
            return Err(Error::CannotParseAsset {
                asset: str.to_string(),
            });
        }
        let code = split[0];
        let issuer = split[1];
        let re = Regex::new("^[[:alnum:]]{1,12}$").unwrap();
        if !re.is_match(code) {
            return Err(Error::InvalidAssetCode {
                asset: str.to_string(),
            });
        }
        if code.len() <= 4 {
            let mut asset_code: [u8; 4] = [0; 4];
            for (i, b) in code.as_bytes().iter().enumerate() {
                asset_code[i] = *b;
            }
            Ok(Asset::CreditAlphanum4(AlphaNum4 {
                asset_code: AssetCode4(asset_code),
                issuer: parse_account_id(issuer)?,
            }))
        } else {
            let mut asset_code: [u8; 12] = [0; 12];
            for (i, b) in code.as_bytes().iter().enumerate() {
                asset_code[i] = *b;
            }
            Ok(Asset::CreditAlphanum12(AlphaNum12 {
                asset_code: AssetCode12(asset_code),
                issuer: parse_account_id(issuer)?,
            }))
        }
    }

    pub fn parse_account_id(str: &str) -> Result<AccountId, Error> {
        let pk_bytes = stellar_strkey::ed25519::PublicKey::from_string(str)
            .map_err(|_| Error::CannotParseAccountId {
                account_id: str.to_string(),
            })?
            .0;
        Ok(AccountId(PublicKey::PublicKeyTypeEd25519(pk_bytes.into())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_id_from_str() {
        // strkey
        match contract_id_from_str("CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE") {
            Ok(contract_id) => assert_eq!(
                contract_id,
                [
                    0x36, 0x3e, 0xaa, 0x38, 0x67, 0x84, 0x1f, 0xba, 0xd0, 0xf4, 0xed, 0x88, 0xc7,
                    0x79, 0xe4, 0xfe, 0x66, 0xe5, 0x6a, 0x24, 0x70, 0xdc, 0x98, 0xc0, 0xec, 0x9c,
                    0x07, 0x3d, 0x05, 0xc7, 0xb1, 0x03,
                ]
            ),
            Err(err) => panic!("Failed to parse contract id: {err}"),
        }

        // hex
        match contract_id_from_str(
            "363eaa3867841fbad0f4ed88c779e4fe66e56a2470dc98c0ec9c073d05c7b103",
        ) {
            Ok(contract_id) => assert_eq!(
                contract_id,
                [
                    0x36, 0x3e, 0xaa, 0x38, 0x67, 0x84, 0x1f, 0xba, 0xd0, 0xf4, 0xed, 0x88, 0xc7,
                    0x79, 0xe4, 0xfe, 0x66, 0xe5, 0x6a, 0x24, 0x70, 0xdc, 0x98, 0xc0, 0xec, 0x9c,
                    0x07, 0x3d, 0x05, 0xc7, 0xb1, 0x03,
                ]
            ),
            Err(err) => panic!("Failed to parse contract id: {err}"),
        }

        // unpadded-hex
        match contract_id_from_str("1") {
            Ok(contract_id) => assert_eq!(
                contract_id,
                [
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
                ]
            ),
            Err(err) => panic!("Failed to parse contract id: {err}"),
        }

        // invalid hex
        match contract_id_from_str("foobar") {
            Ok(_) => panic!("Expected parsing to fail"),
            Err(err) => assert_eq!(err, stellar_strkey::DecodeError::Invalid),
        }

        // hex too long (33 bytes)
        match contract_id_from_str(
            "000000000000000000000000000000000000000000000000000000000000000000",
        ) {
            Ok(_) => panic!("Expected parsing to fail"),
            Err(err) => assert_eq!(err, stellar_strkey::DecodeError::Invalid),
        }
    }
}
