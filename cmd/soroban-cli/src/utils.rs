use ed25519_dalek::Signer;
use hex::FromHexError;
use sha2::{Digest, Sha256};
use soroban_env_host::{
    im_rc::OrdMap,
    storage::{AccessType, Footprint, Storage},
    xdr::{
        AccountEntry, AccountEntryExt, AccountId, ContractCodeEntry, ContractDataEntry,
        DecoratedSignature, Error as XdrError, ExtensionPoint, Hash, InstallContractCodeArgs,
        LedgerEntry, LedgerEntryData, LedgerEntryExt, LedgerFootprint, LedgerKey,
        LedgerKeyContractCode, LedgerKeyContractData, ScContractCode, ScObject, ScSpecEntry,
        ScStatic, ScVal, SequenceNumber, Signature, SignatureHint, StringM, Thresholds,
        Transaction, TransactionEnvelope, TransactionSignaturePayload,
        TransactionSignaturePayloadTaggedTransaction, TransactionV1Envelope, VecM, WriteXdr,
    },
};
use soroban_spec::read::FromWasmError;
use stellar_strkey::StrkeyPrivateKeyEd25519;

pub fn contract_hash(contract: &[u8]) -> Result<Hash, XdrError> {
    let args_xdr = InstallContractCodeArgs {
        code: contract.try_into()?,
    }
    .to_xdr()?;
    Ok(Hash(Sha256::digest(args_xdr).into()))
}

pub fn add_contract_to_ledger_entries(
    entries: &mut OrdMap<LedgerKey, LedgerEntry>,
    contract_id: [u8; 32],
    contract: Vec<u8>,
) -> Result<(), XdrError> {
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
    entries.insert(code_key, code_entry);

    // Create the contract
    let contract_key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: contract_id.into(),
        key: ScVal::Static(ScStatic::LedgerKeyContractCode),
    });

    let contract_entry = LedgerEntry {
        last_modified_ledger_seq: 0,
        data: LedgerEntryData::ContractData(ContractDataEntry {
            contract_id: contract_id.into(),
            key: ScVal::Static(ScStatic::LedgerKeyContractCode),
            val: ScVal::Object(Some(ScObject::ContractCode(ScContractCode::WasmRef(hash)))),
        }),
        ext: LedgerEntryExt::V0,
    };
    entries.insert(contract_key, contract_entry);

    Ok(())
}

pub fn padded_hex_from_str(s: &String, n: usize) -> Result<Vec<u8>, FromHexError> {
    let mut decoded = vec![0u8; n];
    let padded = format!("{s:0>width$}", width = n * 2);
    hex::decode_to_slice(padded, &mut decoded)?;
    Ok(decoded)
}

pub fn transaction_hash(tx: &Transaction, network_passphrase: &str) -> Result<[u8; 32], XdrError> {
    let signature_payload = TransactionSignaturePayload {
        network_id: Hash(Sha256::digest(network_passphrase).into()),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone()),
    };
    Ok(Sha256::digest(signature_payload.to_xdr()?).into())
}

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

pub fn contract_id_from_str(contract_id: &String) -> Result<[u8; 32], FromHexError> {
    padded_hex_from_str(contract_id, 32)?
        .try_into()
        .map_err(|_| FromHexError::InvalidStringLength)
}

pub fn get_contract_spec_from_storage(
    storage: &mut Storage,
    contract_id: [u8; 32],
) -> Result<Vec<ScSpecEntry>, FromWasmError> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: contract_id.into(),
        key: ScVal::Static(ScStatic::LedgerKeyContractCode),
    });
    if let Ok(LedgerEntry {
        data:
            LedgerEntryData::ContractData(ContractDataEntry {
                val: ScVal::Object(Some(ScObject::ContractCode(c))),
                ..
            }),
        ..
    }) = storage.get(&key)
    {
        match c {
            ScContractCode::Token => soroban_spec::read::parse_raw(&soroban_token_spec::spec_xdr())
                .map_err(FromWasmError::Parse),
            ScContractCode::WasmRef(hash) => {
                if let Ok(LedgerEntry {
                    data: LedgerEntryData::ContractCode(ContractCodeEntry { code, .. }),
                    ..
                }) = storage.get(&LedgerKey::ContractCode(LedgerKeyContractCode { hash }))
                {
                    soroban_spec::read::from_wasm(&code)
                } else {
                    Err(FromWasmError::NotFound)
                }
            }
        }
    } else {
        Err(FromWasmError::NotFound)
    }
}

pub fn vec_to_hash(res: &ScVal) -> Result<String, XdrError> {
    if let ScVal::Object(Some(ScObject::Bytes(res_hash))) = &res {
        let mut hash_bytes: [u8; 32] = [0; 32];
        for (i, b) in res_hash.iter().enumerate() {
            hash_bytes[i] = *b;
        }
        Ok(hex::encode(hash_bytes))
    } else {
        Err(XdrError::Invalid)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ParseSecretKeyError {
    #[error("cannot parse secret key")]
    CannotParseSecretKey,
}

pub fn parse_secret_key(strkey: &str) -> Result<ed25519_dalek::Keypair, ParseSecretKeyError> {
    let seed = StrkeyPrivateKeyEd25519::from_string(strkey)
        .map_err(|_| ParseSecretKeyError::CannotParseSecretKey)?;
    let secret_key = ed25519_dalek::SecretKey::from_bytes(&seed.0)
        .map_err(|_| ParseSecretKeyError::CannotParseSecretKey)?;
    let public_key = (&secret_key).into();
    Ok(ed25519_dalek::Keypair {
        secret: secret_key,
        public: public_key,
    })
}

pub fn create_ledger_footprint(footprint: &Footprint) -> LedgerFootprint {
    let mut read_only: Vec<LedgerKey> = vec![];
    let mut read_write: Vec<LedgerKey> = vec![];
    let Footprint(m) = footprint;
    for (k, v) in m {
        let dest = match v {
            AccessType::ReadOnly => &mut read_only,
            AccessType::ReadWrite => &mut read_write,
        };
        dest.push(*k.clone());
    }
    LedgerFootprint {
        read_only: read_only.try_into().unwrap(),
        read_write: read_write.try_into().unwrap(),
    }
}

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
            home_domain: StringM::default(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_secret_key() {
        let seed = "SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP";
        let keypair = parse_secret_key(seed).unwrap();

        let expected_public_key: [u8; 32] = [
            0x31, 0x40, 0xf1, 0x40, 0x99, 0xa7, 0x4c, 0x90, 0xd4, 0x62, 0x48, 0xec, 0x8d, 0xef,
            0xb3, 0x38, 0xc8, 0x2c, 0xe2, 0x42, 0x85, 0xc9, 0xf7, 0xb8, 0x95, 0xce, 0xdd, 0x6f,
            0x96, 0x47, 0x82, 0x96,
        ];
        assert_eq!(expected_public_key, keypair.public.to_bytes());

        let expected_secret_key: [u8; 32] = [
            0x4a, 0x62, 0x97, 0x5f, 0xc7, 0xb9, 0x9a, 0x18, 0xa0, 0x41, 0xba, 0x6, 0x24, 0xd0,
            0x70, 0xf3, 0x95, 0x57, 0x58, 0x82, 0x81, 0x5a, 0x51, 0xbc, 0x3b, 0x49, 0xae, 0x5f,
            0x37, 0x1e, 0x9c, 0x4a,
        ];
        assert_eq!(expected_secret_key, keypair.secret.to_bytes());
    }
}
