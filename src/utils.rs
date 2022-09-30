use hex::FromHexError;
use sha2::{Digest, Sha256};
use soroban_env_host::{
    im_rc::OrdMap,
    storage::Storage,
    xdr::{
        ContractDataEntry, Error as XdrError, Hash, LedgerEntry, LedgerEntryData, LedgerEntryExt,
        LedgerKey, LedgerKeyContractData, ScContractCode, ScObject, ScStatic, ScStatus,
        ScUnknownErrorCode, ScVal, Transaction, TransactionSignaturePayload,
        TransactionSignaturePayloadTaggedTransaction, WriteXdr,
    },
    HostError,
};
use stellar_strkey::StrkeyPrivateKeyEd25519;

pub fn add_contract_to_ledger_entries(
    entries: &mut OrdMap<LedgerKey, LedgerEntry>,
    contract_id: [u8; 32],
    contract: Vec<u8>,
) -> Result<(), XdrError> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: contract_id.into(),
        key: ScVal::Static(ScStatic::LedgerKeyContractCode),
    });

    let data = LedgerEntryData::ContractData(ContractDataEntry {
        contract_id: contract_id.into(),
        key: ScVal::Static(ScStatic::LedgerKeyContractCode),
        val: ScVal::Object(Some(ScObject::ContractCode(ScContractCode::Wasm(
            contract.try_into()?,
        )))),
    });

    let entry = LedgerEntry {
        last_modified_ledger_seq: 0,
        data,
        ext: LedgerEntryExt::V0,
    };

    entries.insert(key, entry);
    Ok(())
}

pub fn padded_hex_from_str(s: &String, n: usize) -> Result<Vec<u8>, FromHexError> {
    let mut decoded = vec![0u8; n];
    let padded = format!("{:0>width$}", s, width = n * 2);
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

pub fn contract_id_from_str(contract_id: &String) -> Result<[u8; 32], FromHexError> {
    padded_hex_from_str(contract_id, 32)?
        .try_into()
        .map_err(|_| FromHexError::InvalidStringLength)
}

pub fn get_contract_wasm_from_storage(
    storage: &mut Storage,
    contract_id: [u8; 32],
) -> Result<Vec<u8>, HostError> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: contract_id.into(),
        key: ScVal::Static(ScStatic::LedgerKeyContractCode),
    });
    if let LedgerEntryData::ContractData(entry) = storage.get(&key)?.data {
        if let ScVal::Object(Some(ScObject::ContractCode(ScContractCode::Wasm(data)))) = entry.val {
            return Ok(data.to_vec());
        }
    }
    Err(HostError::from(ScStatus::UnknownError(
        ScUnknownErrorCode::General,
    )))
}

#[derive(thiserror::Error, Debug)]
pub enum ParsePrivateKeyError {
    #[error("cannot parse private key")]
    CannotParsePrivateKey,
}

pub fn parse_private_key(strkey: &str) -> Result<ed25519_dalek::Keypair, ParsePrivateKeyError> {
    let seed = StrkeyPrivateKeyEd25519::from_string(strkey)
        .map_err(|_| ParsePrivateKeyError::CannotParsePrivateKey)?;
    let secret_key = ed25519_dalek::SecretKey::from_bytes(&seed.0)
        .map_err(|_| ParsePrivateKeyError::CannotParsePrivateKey)?;
    let public_key = (&secret_key).into();
    Ok(ed25519_dalek::Keypair {
        secret: secret_key,
        public: public_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_private_key() {
        let seed = "SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP";
        let keypair = parse_private_key(seed).unwrap();

        let expected_public_key: [u8; 32] = [
            0x31, 0x40, 0xf1, 0x40, 0x99, 0xa7, 0x4c, 0x90, 0xd4, 0x62, 0x48, 0xec, 0x8d, 0xef,
            0xb3, 0x38, 0xc8, 0x2c, 0xe2, 0x42, 0x85, 0xc9, 0xf7, 0xb8, 0x95, 0xce, 0xdd, 0x6f,
            0x96, 0x47, 0x82, 0x96,
        ];
        assert_eq!(expected_public_key, keypair.public.to_bytes());

        let expected_private_key: [u8; 32] = [
            0x4a, 0x62, 0x97, 0x5f, 0xc7, 0xb9, 0x9a, 0x18, 0xa0, 0x41, 0xba, 0x6, 0x24, 0xd0,
            0x70, 0xf3, 0x95, 0x57, 0x58, 0x82, 0x81, 0x5a, 0x51, 0xbc, 0x3b, 0x49, 0xae, 0x5f,
            0x37, 0x1e, 0x9c, 0x4a,
        ];
        assert_eq!(expected_private_key, keypair.secret.to_bytes());
    }
}
