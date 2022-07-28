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
