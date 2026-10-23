use soroban_ledger_snapshot::LedgerSnapshot;
use soroban_spec_tools::contract as contract_spec;
use stellar_xdr::{
    self as xdr, ContractExecutable, ContractExecutableExternalRef, Hash, LedgerEntryData,
    LedgerKey, ScAddress, ScContractInstance, ScSpecEntry, ScVal,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("contract instance not found in snapshot for {0}")]
    ContractInstanceNotFound(ScAddress),
    #[error("contract is a Stellar Asset contract and does not contain a Wasm executable")]
    ContractIsStellarAsset,
    #[error("executable reference entry not found in snapshot for owner {owner}, tag {tag}")]
    ExecutableReferenceNotFound { owner: ScAddress, tag: String },
    #[error("invalid Wasm hash in executable reference entry: expected 32 bytes")]
    InvalidWasmHash,
    #[error("Wasm code not found in snapshot for hash {0}")]
    WasmNotFound(String),
    #[error("parsing contract spec from Wasm: {0}")]
    CannotParseContractSpec(#[from] soroban_spec::read::FromWasmError),
    #[error("contract spec error: {0}")]
    ContractSpec(#[from] contract_spec::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

/// Retrieve the contract instance entry for a contract address from a snapshot.
pub fn get_contract_instance<'a>(
    snapshot: &'a LedgerSnapshot,
    contract_address: &ScAddress,
) -> Option<&'a ScContractInstance> {
    for (key, (entry, _)) in &snapshot.ledger_entries {
        if let LedgerKey::ContractData(k) = key.as_ref() {
            if &k.contract == contract_address && k.key == ScVal::LedgerKeyContractInstance {
                if let LedgerEntryData::ContractData(data) = &entry.data {
                    if let ScVal::ContractInstance(instance) = &data.val {
                        return Some(instance);
                    }
                }
            }
        }
    }
    None
}

/// Resolve a CAP-85 externally managed executable to the 32-byte Wasm hash stored in the snapshot.
pub fn resolve_external_ref_wasm_hash(
    snapshot: &LedgerSnapshot,
    external_ref: &ContractExecutableExternalRef,
) -> Result<Hash, Error> {
    for (key, (entry, _)) in &snapshot.ledger_entries {
        if let LedgerKey::ContractData(k) = key.as_ref() {
            if k.contract == external_ref.executable_owner
                && k.key == ScVal::ExecutableTag(external_ref.tag.clone())
            {
                if let LedgerEntryData::ContractData(data) = &entry.data {
                    if let ScVal::Bytes(bytes) = &data.val {
                        let hash_bytes: [u8; 32] = bytes
                            .as_slice()
                            .try_into()
                            .map_err(|_| Error::InvalidWasmHash)?;
                        return Ok(Hash(hash_bytes));
                    }
                }
            }
        }
    }
    Err(Error::ExecutableReferenceNotFound {
        owner: external_ref.executable_owner.clone(),
        tag: soroban_spec_tools::sanitize(&String::from_utf8_lossy(external_ref.tag.0.as_slice())),
    })
}

/// Retrieve the Wasm bytecode for a given Wasm hash from the snapshot.
pub fn get_wasm_from_hash<'a>(
    snapshot: &'a LedgerSnapshot,
    hash: &Hash,
) -> Result<&'a [u8], Error> {
    for (key, (entry, _)) in &snapshot.ledger_entries {
        if let LedgerKey::ContractCode(k) = key.as_ref() {
            if &k.hash == hash {
                if let LedgerEntryData::ContractCode(data) = &entry.data {
                    return Ok(data.code.as_slice());
                }
            }
        }
    }
    Err(Error::WasmNotFound(hex::encode(hash)))
}

/// Resolve the Wasm hash for a given contract address from the snapshot across all supported
/// `ContractExecutable` variants (`Wasm`, `ExternalRef`, `StellarAsset`).
pub fn get_wasm_hash_for_contract(
    snapshot: &LedgerSnapshot,
    contract_address: &ScAddress,
) -> Result<Hash, Error> {
    let instance = get_contract_instance(snapshot, contract_address)
        .ok_or_else(|| Error::ContractInstanceNotFound(contract_address.clone()))?;
    match &instance.executable {
        ContractExecutable::Wasm(hash) => Ok(hash.clone()),
        ContractExecutable::StellarAsset => Err(Error::ContractIsStellarAsset),
        ContractExecutable::ExternalRef(external_ref) => {
            resolve_external_ref_wasm_hash(snapshot, external_ref)
        }
    }
}

/// Retrieve the contract Wasm bytecode for a given contract address from the snapshot.
pub fn get_contract_wasm<'a>(
    snapshot: &'a LedgerSnapshot,
    contract_address: &ScAddress,
) -> Result<&'a [u8], Error> {
    let hash = get_wasm_hash_for_contract(snapshot, contract_address)?;
    get_wasm_from_hash(snapshot, &hash)
}

/// Retrieve the contract spec entries for a contract address from the snapshot across all
/// `ContractExecutable` variants (`StellarAsset`, `Wasm`, `ExternalRef`).
pub fn get_contract_spec(
    snapshot: &LedgerSnapshot,
    contract_address: &ScAddress,
) -> Result<Vec<ScSpecEntry>, Error> {
    let instance = get_contract_instance(snapshot, contract_address)
        .ok_or_else(|| Error::ContractInstanceNotFound(contract_address.clone()))?;
    match &instance.executable {
        ContractExecutable::StellarAsset => {
            Ok(soroban_spec::read::parse_raw(&stellar_asset_spec::xdr())?)
        }
        ContractExecutable::Wasm(hash) => {
            let wasm = get_wasm_from_hash(snapshot, hash)?;
            let spec = contract_spec::Spec::new(wasm)?.spec;
            Ok(spec)
        }
        ContractExecutable::ExternalRef(external_ref) => {
            let hash = resolve_external_ref_wasm_hash(snapshot, external_ref)?;
            let wasm = get_wasm_from_hash(snapshot, &hash)?;
            let spec = contract_spec::Spec::new(wasm)?.spec;
            Ok(spec)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::{
        ContractCodeEntry, ContractCodeEntryExt, ContractDataDurability, ContractDataEntry,
        ContractId, LedgerEntry, LedgerEntryExt, LedgerKeyContractCode, LedgerKeyContractData,
        ScBytes, ScString,
    };

    fn dummy_snapshot() -> LedgerSnapshot {
        LedgerSnapshot {
            protocol_version: 22,
            sequence_number: 100,
            timestamp: 1_700_000_000,
            network_id: [0u8; 32],
            base_reserve: 10_000_000,
            min_persistent_entry_ttl: 100,
            min_temp_entry_ttl: 100,
            max_entry_ttl: 10_000,
            ledger_entries: Vec::new(),
        }
    }

    fn make_ledger_entry(data: LedgerEntryData) -> LedgerEntry {
        LedgerEntry {
            last_modified_ledger_seq: 100,
            data,
            ext: LedgerEntryExt::V0,
        }
    }

    #[test]
    fn test_stellar_asset_spec() {
        let mut snapshot = dummy_snapshot();
        let contract_address = ScAddress::Contract(ContractId(Hash([1u8; 32])));

        let key = LedgerKey::ContractData(LedgerKeyContractData {
            contract: contract_address.clone(),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
        });

        let data = LedgerEntryData::ContractData(ContractDataEntry {
            contract: contract_address.clone(),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
            val: ScVal::ContractInstance(ScContractInstance {
                executable: ContractExecutable::StellarAsset,
                storage: None,
            }),
            ext: xdr::ExtensionPoint::V0,
        });

        snapshot
            .ledger_entries
            .push((Box::new(key), (Box::new(make_ledger_entry(data)), None)));

        // Spec resolves successfully to built-in asset spec
        let spec = get_contract_spec(&snapshot, &contract_address).unwrap();
        assert!(!spec.is_empty());

        // Resolving wasm hash for StellarAsset returns ContractIsStellarAsset error
        match get_wasm_hash_for_contract(&snapshot, &contract_address) {
            Err(Error::ContractIsStellarAsset) => {}
            other => panic!("expected ContractIsStellarAsset, got {:?}", other),
        }
    }

    #[test]
    fn test_wasm_and_external_ref_resolution() {
        let mut snapshot = dummy_snapshot();

        let wasm_hash = Hash([42u8; 32]);
        let raw_wasm_code = b"\x00asm\x01\x00\x00\x00".to_vec();

        // 1. Add Wasm ContractCode entry
        let code_key = LedgerKey::ContractCode(LedgerKeyContractCode {
            hash: wasm_hash.clone(),
        });
        let code_data = LedgerEntryData::ContractCode(ContractCodeEntry {
            hash: wasm_hash.clone(),
            code: raw_wasm_code.clone().try_into().unwrap(),
            ext: ContractCodeEntryExt::V0,
        });
        snapshot.ledger_entries.push((
            Box::new(code_key),
            (Box::new(make_ledger_entry(code_data)), None),
        ));

        // 2. Add Direct Wasm Contract Instance
        let direct_contract = ScAddress::Contract(ContractId(Hash([2u8; 32])));
        let direct_key = LedgerKey::ContractData(LedgerKeyContractData {
            contract: direct_contract.clone(),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
        });
        let direct_data = LedgerEntryData::ContractData(ContractDataEntry {
            contract: direct_contract.clone(),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
            val: ScVal::ContractInstance(ScContractInstance {
                executable: ContractExecutable::Wasm(wasm_hash.clone()),
                storage: None,
            }),
            ext: xdr::ExtensionPoint::V0,
        });
        snapshot.ledger_entries.push((
            Box::new(direct_key),
            (Box::new(make_ledger_entry(direct_data)), None),
        ));

        // Direct contract wasm hash & bytecode verification
        let resolved_hash = get_wasm_hash_for_contract(&snapshot, &direct_contract).unwrap();
        assert_eq!(resolved_hash, wasm_hash);
        let wasm_bytes = get_contract_wasm(&snapshot, &direct_contract).unwrap();
        assert_eq!(wasm_bytes, raw_wasm_code.as_slice());

        // 3. Add ExternalRef Contract Instance pointing to an owner contract
        let owner_contract = ScAddress::Contract(ContractId(Hash([3u8; 32])));
        let tag = ScString("v1.0".try_into().unwrap());
        let proxy_contract = ScAddress::Contract(ContractId(Hash([4u8; 32])));

        let proxy_key = LedgerKey::ContractData(LedgerKeyContractData {
            contract: proxy_contract.clone(),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
        });
        let proxy_data = LedgerEntryData::ContractData(ContractDataEntry {
            contract: proxy_contract.clone(),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
            val: ScVal::ContractInstance(ScContractInstance {
                executable: ContractExecutable::ExternalRef(ContractExecutableExternalRef {
                    executable_owner: owner_contract.clone(),
                    tag: tag.clone(),
                }),
                storage: None,
            }),
            ext: xdr::ExtensionPoint::V0,
        });
        snapshot.ledger_entries.push((
            Box::new(proxy_key),
            (Box::new(make_ledger_entry(proxy_data)), None),
        ));

        // 4. Add the Executable Reference Entry on owner_contract
        let ref_key = LedgerKey::ContractData(LedgerKeyContractData {
            contract: owner_contract.clone(),
            key: ScVal::ExecutableTag(tag.clone()),
            durability: ContractDataDurability::Persistent,
        });
        let ref_data = LedgerEntryData::ContractData(ContractDataEntry {
            contract: owner_contract.clone(),
            key: ScVal::ExecutableTag(tag),
            durability: ContractDataDurability::Persistent,
            val: ScVal::Bytes(ScBytes(wasm_hash.0.to_vec().try_into().unwrap())),
            ext: xdr::ExtensionPoint::V0,
        });
        snapshot.ledger_entries.push((
            Box::new(ref_key),
            (Box::new(make_ledger_entry(ref_data)), None),
        ));

        // Proxy contract ExternalRef two-hop resolution verification
        let proxy_resolved_hash = get_wasm_hash_for_contract(&snapshot, &proxy_contract).unwrap();
        assert_eq!(proxy_resolved_hash, wasm_hash);
        let proxy_wasm = get_contract_wasm(&snapshot, &proxy_contract).unwrap();
        assert_eq!(proxy_wasm, raw_wasm_code.as_slice());
    }

    #[test]
    fn test_missing_contract_instance() {
        let snapshot = dummy_snapshot();
        let nonexistent = ScAddress::Contract(ContractId(Hash([99u8; 32])));
        match get_wasm_hash_for_contract(&snapshot, &nonexistent) {
            Err(Error::ContractInstanceNotFound(addr)) => assert_eq!(addr, nonexistent),
            other => panic!("expected ContractInstanceNotFound, got {:?}", other),
        }
    }
}
