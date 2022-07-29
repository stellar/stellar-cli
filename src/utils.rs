use hex::FromHexError;
use soroban_env_host::{
    im_rc::OrdMap,
    storage::Storage,
    xdr::{
        ContractDataEntry, Error as XdrError, LedgerEntry, LedgerEntryData, LedgerEntryExt,
        LedgerKey, LedgerKeyContractData, ScObject, ScStatic, ScStatus, ScUnknownErrorCode, ScVal,
    },
    HostError,
};

pub fn add_contract_to_ledger_entries(
    entries: &mut OrdMap<LedgerKey, LedgerEntry>,
    contract_id: [u8; 32],
    contract: Vec<u8>,
) -> Result<(), XdrError> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: contract_id.into(),
        key: ScVal::Static(ScStatic::LedgerKeyContractCodeWasm),
    });

    let data = LedgerEntryData::ContractData(ContractDataEntry {
        contract_id: contract_id.into(),
        key: ScVal::Static(ScStatic::LedgerKeyContractCodeWasm),
        val: ScVal::Object(Some(ScObject::Binary(contract.try_into()?))),
    });

    let entry = LedgerEntry {
        last_modified_ledger_seq: 0,
        data,
        ext: LedgerEntryExt::V0,
    };

    entries.insert(key, entry);
    Ok(())
}

pub fn contract_id_from_str(contract_id: &String) -> Result<[u8; 32], FromHexError> {
    let mut decoded = [0u8; 32];
    let padded = format!("{:0>width$}", contract_id, width = decoded.len() * 2);
    hex::decode_to_slice(padded, &mut decoded)?;
    Ok(decoded)
}

pub fn get_contract_wasm_from_storage(
    storage: &mut Storage,
    contract_id: [u8; 32],
) -> Result<Vec<u8>, HostError> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract_id: contract_id.into(),
        key: ScVal::Static(ScStatic::LedgerKeyContractCodeWasm),
    });
    if let LedgerEntryData::ContractData(entry) = storage.get(&key)?.data {
        if let ScVal::Object(Some(ScObject::Binary(data))) = entry.val {
            return Ok(data.to_vec());
        }
    }
    Err(HostError::from(ScStatus::UnknownError(
        ScUnknownErrorCode::General,
    )))
}
