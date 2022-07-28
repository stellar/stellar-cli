use hex::FromHexError;
use soroban_env_host::{
    im_rc::OrdMap,
    xdr::{
        ContractDataEntry, Error as XdrError, LedgerEntry, LedgerEntryData, LedgerEntryExt,
        LedgerKey, LedgerKeyContractData, ScObject, ScStatic, ScVal,
    },
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
    const CONTRACT_ID_LENGTH: usize = 32;
    let contract_id_prefix = hex::decode(contract_id)?;
    if contract_id_prefix.len() > CONTRACT_ID_LENGTH {
        return Err(FromHexError::InvalidStringLength);
    }
    let mut contract_id = [0u8; CONTRACT_ID_LENGTH];
    contract_id[..contract_id_prefix.len()].copy_from_slice(contract_id_prefix.as_slice());
    Ok(contract_id)
}
