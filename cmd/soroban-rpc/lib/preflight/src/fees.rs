use ledger_storage;
use soroban_env_host::budget::Budget;
use soroban_env_host::fees::{
    compute_transaction_resource_fee, FeeConfiguration, TransactionResources,
};
use soroban_env_host::storage::{AccessType, Footprint, Storage, StorageMap};
use soroban_env_host::xdr;
use soroban_env_host::xdr::{
    BumpFootprintExpirationOp, ConfigSettingEntry, ConfigSettingId, DecoratedSignature,
    DiagnosticEvent, ExtensionPoint, InvokeHostFunctionOp, LedgerEntry, LedgerEntryData,
    LedgerFootprint, LedgerKey, LedgerKeyConfigSetting, Memo, MuxedAccount, MuxedAccountMed25519,
    Operation, OperationBody, Preconditions, RestoreFootprintOp, SequenceNumber, Signature,
    SignatureHint, SorobanResources, SorobanTransactionData, Transaction, TransactionExt,
    TransactionV1Envelope, Uint256, WriteXdr,
};
use std::cmp::max;
use std::convert::{TryFrom, TryInto};
use std::error;

pub(crate) fn compute_host_function_transaction_data_and_min_fee(
    op: &InvokeHostFunctionOp,
    snapshot_source: &ledger_storage::LedgerStorage,
    storage: &Storage,
    budget: &Budget,
    events: &Vec<DiagnosticEvent>,
) -> Result<(SorobanTransactionData, i64), Box<dyn error::Error>> {
    let soroban_resources =
        calculate_host_function_soroban_resources(snapshot_source, storage, budget, events)?;
    let fee_configuration = get_fee_configuration(snapshot_source)?;

    let read_write_entries = u32::try_from(soroban_resources.footprint.read_write.as_vec().len())?;

    let transaction_resources = TransactionResources {
        instructions: soroban_resources.instructions,
        read_entries: u32::try_from(soroban_resources.footprint.read_only.as_vec().len())?
            + read_write_entries,
        write_entries: read_write_entries,
        read_bytes: soroban_resources.read_bytes,
        write_bytes: soroban_resources.write_bytes,
        metadata_size_bytes: soroban_resources.extended_meta_data_size_bytes,
        // Note: we could get a better transaction size if the full transaction was passed down to libpreflight
        transaction_size_bytes: estimate_max_transaction_size_for_operation(
            &OperationBody::InvokeHostFunction(op.clone()),
            &soroban_resources.footprint,
        )?,
    };
    let (min_fee, ref_fee) =
        compute_transaction_resource_fee(&transaction_resources, &fee_configuration);
    let transaction_data = SorobanTransactionData {
        resources: soroban_resources,
        refundable_fee: ref_fee,
        ext: ExtensionPoint::V0,
    };
    Ok((transaction_data, min_fee))
}

fn estimate_max_transaction_size_for_operation(
    op: &OperationBody,
    fp: &LedgerFootprint,
) -> Result<u32, Box<dyn error::Error>> {
    let source = MuxedAccount::MuxedEd25519(MuxedAccountMed25519 {
        id: 0,
        ed25519: Uint256([0; 32]),
    });
    // generate the maximum memo size and signature size
    // TODO: is this being too conservative?
    let memo_text: Vec<u8> = [0; 28].into();
    let signatures: Vec<DecoratedSignature> = vec![
        DecoratedSignature {
            hint: SignatureHint([0; 4]),
            signature: Signature::default(),
        };
        20
    ];
    let envelope = TransactionV1Envelope {
        tx: Transaction {
            source_account: source.clone(),
            fee: 0,
            seq_num: SequenceNumber(0),
            cond: Preconditions::None,
            memo: Memo::Text(memo_text.try_into()?),
            operations: vec![Operation {
                source_account: Some(source),
                body: op.clone(),
            }]
            .try_into()?,
            ext: TransactionExt::V1(SorobanTransactionData {
                resources: SorobanResources {
                    footprint: fp.clone(),
                    instructions: 0,
                    read_bytes: 0,
                    write_bytes: 0,
                    extended_meta_data_size_bytes: 0,
                },
                refundable_fee: 0,
                ext: ExtensionPoint::V0,
            }),
        },
        signatures: signatures.try_into()?,
    };

    let envelope_xdr = envelope.to_xdr()?;
    let envelope_size = envelope_xdr.len();

    // Add a 15% leeway
    let envelope_size = envelope_size * 115 / 100;
    Ok(u32::try_from(envelope_size)?)
}

fn calculate_host_function_soroban_resources(
    snapshot_source: &ledger_storage::LedgerStorage,
    storage: &Storage,
    budget: &Budget,
    events: &Vec<DiagnosticEvent>,
) -> Result<SorobanResources, Box<dyn error::Error>> {
    let fp = storage_footprint_to_ledger_footprint(&storage.footprint)?;
    /*
      readBytes = size(footprint.readOnly) + size(footprint.readWrite)
      writeBytes = size(storage.map[rw entries])
      metadataSize = readBytes(footprint.readWrite) + writeBytes + eventsSize
    */
    let original_write_ledger_entry_bytes =
        calculate_unmodified_ledger_entry_bytes(fp.read_write.as_vec(), snapshot_source, false)?;
    let read_bytes =
        calculate_unmodified_ledger_entry_bytes(fp.read_only.as_vec(), snapshot_source, false)?
            + original_write_ledger_entry_bytes;
    let write_bytes =
        calculate_modified_read_write_ledger_entry_bytes(&storage.footprint, &storage.map, budget)?;
    let meta_data_size_bytes =
        original_write_ledger_entry_bytes + write_bytes + calculate_event_size_bytes(events)?;

    // Add a 15% leeway with a minimum of 50k instructions
    let instructions = max(
        budget.get_cpu_insns_consumed() + 50000,
        budget.get_cpu_insns_consumed() * 115 / 100,
    );
    Ok(SorobanResources {
        footprint: fp,
        instructions: u32::try_from(instructions)?,
        read_bytes,
        write_bytes,
        extended_meta_data_size_bytes: meta_data_size_bytes,
    })
}

fn get_configuration_setting(
    ledger_storage: &ledger_storage::LedgerStorage,
    setting_id: ConfigSettingId,
) -> Result<ConfigSettingEntry, Box<dyn error::Error>> {
    let key = LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
        config_setting_id: setting_id,
    });
    match ledger_storage.get(&key, false)? {
        LedgerEntry {
            data: LedgerEntryData::ConfigSetting(cs),
            ..
        } => Ok(cs),
        _ => Err(format!(
            "get_configuration_setting(): unexpected ledger entry for {} key",
            setting_id.name()
        )
        .into()),
    }
}

fn get_fee_configuration(
    ledger_storage: &ledger_storage::LedgerStorage,
) -> Result<FeeConfiguration, Box<dyn error::Error>> {
    // TODO: to improve the performance of this function (which is invoked every single preflight call) we can
    //       1. modify ledger_storage.get() so that it can gather multiple entries at once
    //       2. implement a write-through cache for the configuration ledger entries (i.e. the cache is written to at the
    //          same time as the DB, ensuring they are always in memory).
    //

    let ConfigSettingEntry::ContractComputeV0(compute) = get_configuration_setting(ledger_storage, ConfigSettingId::ContractComputeV0)? else {
            return Err(
                "get_fee_configuration(): unexpected config setting entry for ComputeV0 key".into(),
            );
        };

    let ConfigSettingEntry::ContractLedgerCostV0(ledger_cost) = get_configuration_setting(ledger_storage, ConfigSettingId::ContractLedgerCostV0)? else {
        return Err(
            "get_fee_configuration(): unexpected config setting entry for LedgerCostV0 key".into(),
        );
    };

    let ConfigSettingEntry::ContractHistoricalDataV0(historical_data) = get_configuration_setting(ledger_storage, ConfigSettingId::ContractHistoricalDataV0)? else {
        return Err(
            "get_fee_configuration(): unexpected config setting entry for HistoricalDataV0 key".into(),
        );
    };

    let ConfigSettingEntry::ContractMetaDataV0(metadata) = get_configuration_setting(ledger_storage, ConfigSettingId::ContractMetaDataV0)? else {
        return Err(
            "get_fee_configuration(): unexpected config setting entry for MetaDataV0 key".into(),
        );
    };

    let ConfigSettingEntry::ContractBandwidthV0(bandwidth) = get_configuration_setting(ledger_storage, ConfigSettingId::ContractBandwidthV0)? else {
        return Err(
            "get_fee_configuration(): unexpected config setting entry for BandwidthV0 key".into(),
        );
    };

    // Taken from Stellar Core's InitialSorobanNetworkConfig in NetworkConfig.h
    let fee_configuration = FeeConfiguration {
        fee_per_instruction_increment: compute.fee_rate_per_instructions_increment,
        fee_per_read_entry: ledger_cost.fee_read_ledger_entry,
        fee_per_write_entry: ledger_cost.fee_write_ledger_entry,
        fee_per_read_1kb: ledger_cost.fee_read1_kb,
        fee_per_write_1kb: ledger_cost.fee_write1_kb,
        fee_per_historical_1kb: historical_data.fee_historical1_kb,
        fee_per_metadata_1kb: metadata.fee_extended_meta_data1_kb,
        fee_per_propagate_1kb: bandwidth.fee_propagate_data1_kb,
    };
    Ok(fee_configuration)
}

fn calculate_modified_read_write_ledger_entry_bytes(
    footprint: &Footprint,
    storage_map: &StorageMap,
    budget: &Budget,
) -> Result<u32, Box<dyn error::Error>> {
    let mut res: u32 = 0;
    for (lk, ole) in storage_map {
        match footprint.0.get::<LedgerKey>(lk, budget)? {
            Some(AccessType::ReadOnly) => (),
            Some(AccessType::ReadWrite) => {
                if let Some(le) = ole {
                    let entry_bytes = le.to_xdr()?;
                    let key_bytes = lk.to_xdr()?;
                    res += u32::try_from(entry_bytes.len() + key_bytes.len())?;
                }
            }
            None => return Err("storage ledger entry not found in footprint".into()),
        }
    }
    Ok(res)
}

fn calculate_unmodified_ledger_entry_bytes(
    ledger_entries: &Vec<LedgerKey>,
    snapshot_source: &ledger_storage::LedgerStorage,
    include_expired: bool,
) -> Result<u32, Box<dyn error::Error>> {
    let mut res: u32 = 0;
    for lk in ledger_entries {
        res += u32::try_from(lk.to_xdr()?.len())?;
        match snapshot_source.get_xdr(lk, include_expired) {
            Ok(entry_bytes) => {
                res += u32::try_from(entry_bytes.len())?;
            }
            Err(e) => {
                match e {
                    ledger_storage::Error::NotFound =>
                    // The entry is not present in the unmodified ledger storage.
                    // We assume it to be due to the entry being created by a host function invocation.
                    // Thus, we shouldn't count it in as unmodified.
                    {
                        continue;
                    }
                    _ => return Err(e)?,
                }
            }
        };
    }
    Ok(res)
}

fn calculate_event_size_bytes(events: &Vec<DiagnosticEvent>) -> Result<u32, Box<dyn error::Error>> {
    let mut res: u32 = 0;
    for e in events {
        let event_xdr = e.to_xdr()?;
        res += u32::try_from(event_xdr.len())?;
    }
    Ok(res)
}

fn storage_footprint_to_ledger_footprint(foot: &Footprint) -> Result<LedgerFootprint, xdr::Error> {
    let mut read_only: Vec<LedgerKey> = Vec::new();
    let mut read_write: Vec<LedgerKey> = Vec::new();
    for (k, v) in &foot.0 {
        match v {
            AccessType::ReadOnly => read_only.push((**k).clone()),
            AccessType::ReadWrite => read_write.push((**k).clone()),
        }
    }
    Ok(LedgerFootprint {
        read_only: read_only.try_into()?,
        read_write: read_write.try_into()?,
    })
}

pub(crate) fn compute_bump_footprint_exp_transaction_data_and_min_fee(
    footprint: LedgerFootprint,
    ledgers_to_expire: u32,
    snapshot_source: &ledger_storage::LedgerStorage,
) -> Result<(SorobanTransactionData, i64), Box<dyn error::Error>> {
    let read_bytes = calculate_unmodified_ledger_entry_bytes(
        footprint.read_only.as_vec(),
        snapshot_source,
        false,
    )?;
    let soroban_resources = SorobanResources {
        footprint,
        instructions: 0,
        read_bytes,
        write_bytes: 0,
        extended_meta_data_size_bytes: 2 * read_bytes,
    };
    let transaction_resources = TransactionResources {
        instructions: 0,
        read_entries: u32::try_from(soroban_resources.footprint.read_only.as_vec().len())?,
        write_entries: 0,
        read_bytes: soroban_resources.read_bytes,
        write_bytes: 0,
        metadata_size_bytes: soroban_resources.extended_meta_data_size_bytes,
        transaction_size_bytes: estimate_max_transaction_size_for_operation(
            &OperationBody::BumpFootprintExpiration(BumpFootprintExpirationOp {
                ext: ExtensionPoint::V0,
                ledgers_to_expire: ledgers_to_expire,
            }),
            &soroban_resources.footprint,
        )?,
    };
    let fee_configuration = get_fee_configuration(snapshot_source)?;
    let (min_fee, ref_fee) =
        compute_transaction_resource_fee(&transaction_resources, &fee_configuration);
    let transaction_data = SorobanTransactionData {
        resources: soroban_resources,
        refundable_fee: ref_fee,
        ext: ExtensionPoint::V0,
    };
    Ok((transaction_data, min_fee))
}

pub(crate) fn compute_restore_footprint_transaction_data_and_min_fee(
    footprint: LedgerFootprint,
    snapshot_source: &ledger_storage::LedgerStorage,
) -> Result<(SorobanTransactionData, i64), Box<dyn error::Error>> {
    let write_bytes = calculate_unmodified_ledger_entry_bytes(
        footprint.read_write.as_vec(),
        snapshot_source,
        true,
    )?;
    let soroban_resources = SorobanResources {
        footprint,
        instructions: 0,
        // FIXME(fons): this seems to be a workaround a bug in code (the fix is to also count bytes read but not written in readBytes).
        //        we should review it in preview 11.
        read_bytes: write_bytes,
        write_bytes,
        extended_meta_data_size_bytes: 2 * write_bytes,
    };
    let entry_count = u32::try_from(soroban_resources.footprint.read_write.as_vec().len())?;
    let transaction_resources = TransactionResources {
        instructions: 0,
        read_entries: entry_count,
        write_entries: entry_count,
        read_bytes: soroban_resources.read_bytes,
        write_bytes: soroban_resources.write_bytes,
        metadata_size_bytes: soroban_resources.extended_meta_data_size_bytes,
        transaction_size_bytes: estimate_max_transaction_size_for_operation(
            &OperationBody::RestoreFootprint(RestoreFootprintOp {
                ext: ExtensionPoint::V0,
            }),
            &soroban_resources.footprint,
        )?,
    };
    let fee_configuration = get_fee_configuration(snapshot_source)?;
    let (min_fee, ref_fee) =
        compute_transaction_resource_fee(&transaction_resources, &fee_configuration);
    let transaction_data = SorobanTransactionData {
        resources: soroban_resources,
        refundable_fee: ref_fee,
        ext: ExtensionPoint::V0,
    };
    Ok((transaction_data, min_fee))
}
