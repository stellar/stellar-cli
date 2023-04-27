use std::{io::ErrorKind, path::Path};

use ed25519_dalek::Signer;
use hex::FromHexError;
use sha2::{Digest, Sha256};
use soroban_env_host::{
    budget::Budget,
    storage::{AccessType, Footprint, Storage},
    xdr::{
        AccountEntry, AccountEntryExt, AccountId, ContractCodeEntry, ContractDataEntry,
        DecoratedSignature, Error as XdrError, ExtensionPoint, Hash, InstallContractCodeArgs,
        LedgerEntry, LedgerEntryData, LedgerEntryExt, LedgerFootprint, LedgerKey,
        LedgerKeyContractCode, LedgerKeyContractData, ScContractExecutable, ScSpecEntry, ScVal,
        SequenceNumber, Signature, SignatureHint, String32, Thresholds, Transaction,
        TransactionEnvelope, TransactionSignaturePayload,
        TransactionSignaturePayloadTaggedTransaction, TransactionV1Envelope, VecM, WriteXdr,
    },
};
use soroban_ledger_snapshot::LedgerSnapshot;
use soroban_sdk::token::Spec;
use soroban_spec::read::FromWasmError;
use stellar_strkey::ed25519::PrivateKey;

use crate::network::sandbox_network_id;

/// # Errors
///
/// Might return an error
pub fn contract_hash(contract: &[u8]) -> Result<Hash, XdrError> {
    let args_xdr = InstallContractCodeArgs {
        code: contract.try_into()?,
    }
    .to_xdr()?;
    Ok(Hash(Sha256::digest(args_xdr).into()))
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
) -> Result<Hash, XdrError> {
    // Install the code
    let hash = contract_hash(contract.as_slice())?;
    let code_key = LedgerKey::ContractCode(LedgerKeyContractCode { hash: hash.clone() });
    let code_entry = LedgerEntry {
        last_modified_ledger_seq: 0,
        data: LedgerEntryData::ContractCode(ContractCodeEntry {
            code: contract.try_into()?,
            ext: ExtensionPoint::V0,
            hash: hash.clone(),
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
) {
    // Create the contract
    let contract_key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: contract_id.into(),
        key: ScVal::LedgerKeyContractExecutable,
    });

    let contract_entry = LedgerEntry {
        last_modified_ledger_seq: 0,
        data: LedgerEntryData::ContractData(ContractDataEntry {
            contract_id: contract_id.into(),
            key: ScVal::LedgerKeyContractExecutable,
            val: ScVal::ContractExecutable(ScContractExecutable::WasmRef(Hash(wasm_hash))),
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

/// # Errors
///
/// Might return an error
pub fn padded_hex_from_str(s: &String, n: usize) -> Result<Vec<u8>, FromHexError> {
    let mut decoded = vec![0u8; n];
    let padded = format!("{s:0>width$}", width = n * 2);
    hex::decode_to_slice(padded, &mut decoded)?;
    Ok(decoded)
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
pub fn id_from_str<const N: usize>(contract_id: &String) -> Result<[u8; N], FromHexError> {
    padded_hex_from_str(contract_id, N)?
        .try_into()
        .map_err(|_| FromHexError::InvalidStringLength)
}

/// # Errors
///
/// Might return an error
pub fn get_contract_spec_from_storage(
    storage: &mut Storage,
    contract_id: [u8; 32],
) -> Result<Vec<ScSpecEntry>, FromWasmError> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: contract_id.into(),
        key: ScVal::LedgerKeyContractExecutable,
    });
    match storage.get(&key.into(), &Budget::default()) {
        Ok(rc) => match rc.as_ref() {
            LedgerEntry {
                data:
                    LedgerEntryData::ContractData(ContractDataEntry {
                        val: ScVal::ContractExecutable(c),
                        ..
                    }),
                ..
            } => match c {
                ScContractExecutable::Token => {
                    let res = soroban_spec::read::parse_raw(&Spec::spec_xdr());
                    res.map_err(FromWasmError::Parse)
                }
                ScContractExecutable::WasmRef(hash) => {
                    if let Ok(rc) = storage.get(
                        &LedgerKey::ContractCode(LedgerKeyContractCode { hash: hash.clone() })
                            .into(),
                        &Budget::default(),
                    ) {
                        match rc.as_ref() {
                            LedgerEntry {
                                data: LedgerEntryData::ContractCode(ContractCodeEntry { code, .. }),
                                ..
                            } => soroban_spec::read::from_wasm(code),
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

/// # Errors
///
/// Might return an error
pub fn vec_to_hash(res: &ScVal) -> Result<String, XdrError> {
    if let ScVal::Bytes(res_hash) = &res {
        let mut hash_bytes: [u8; 32] = [0; 32];
        for (i, b) in res_hash.iter().enumerate() {
            hash_bytes[i] = *b;
        }
        Ok(hex::encode(hash_bytes))
    } else {
        Err(XdrError::Invalid)
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
