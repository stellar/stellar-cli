use anyhow::{bail, ensure, Context, Error, Result};
use ledger_storage::LedgerStorage;
use soroban_env_host::budget::Budget;
use soroban_env_host::e2e_invoke::{extract_rent_changes, get_ledger_changes, LedgerEntryChange};
use soroban_env_host::fees::{
    compute_rent_fee, compute_transaction_resource_fee, compute_write_fee_per_1kb,
    FeeConfiguration, LedgerEntryRentChange, RentFeeConfiguration, TransactionResources,
    WriteFeeConfiguration,
};
use soroban_env_host::storage::{AccessType, Footprint, Storage};
use soroban_env_host::xdr;
use soroban_env_host::xdr::ContractDataDurability::Persistent;
use soroban_env_host::xdr::{
    BumpFootprintExpirationOp, ConfigSettingEntry, ConfigSettingId, ContractEventType,
    DecoratedSignature, DiagnosticEvent, ExtensionPoint, InvokeHostFunctionOp, LedgerFootprint,
    LedgerKey, Memo, MuxedAccount, MuxedAccountMed25519, Operation, OperationBody, Preconditions,
    RestoreFootprintOp, ScVal, SequenceNumber, Signature, SignatureHint, SorobanResources,
    SorobanTransactionData, Transaction, TransactionExt, TransactionV1Envelope, Uint256, WriteXdr,
};
use state_expiration::{get_restored_ledger_sequence, ExpirableLedgerEntry};
use std::cmp::max;
use std::convert::{TryFrom, TryInto};

pub(crate) fn compute_host_function_transaction_data_and_min_fee(
    op: &InvokeHostFunctionOp,
    pre_storage: &LedgerStorage,
    post_storage: &Storage,
    budget: &Budget,
    events: &Vec<DiagnosticEvent>,
    invocation_result: &ScVal,
    bucket_list_size: u64,
    current_ledger_seq: u32,
) -> Result<(SorobanTransactionData, i64)> {
    let ledger_changes = get_ledger_changes(budget, post_storage, pre_storage)?;
    let soroban_resources =
        calculate_host_function_soroban_resources(&ledger_changes, &post_storage.footprint, budget)
            .context("cannot compute host function resources")?;

    let read_write_entries = u32::try_from(soroban_resources.footprint.read_write.as_vec().len())?;

    let contract_events_size =
        calculate_contract_events_size_bytes(events).context("cannot calculate events size")?;
    let invocation_return_size = u32::try_from(invocation_result.to_xdr()?.len())?;
    // This is totally unintuitive, but it's what's expected by the library
    let final_contract_events_size = contract_events_size + invocation_return_size;

    let transaction_resources = TransactionResources {
        instructions: soroban_resources.instructions,
        read_entries: u32::try_from(soroban_resources.footprint.read_only.as_vec().len())?
            + read_write_entries,
        write_entries: read_write_entries,
        read_bytes: soroban_resources.read_bytes,
        write_bytes: soroban_resources.write_bytes,
        // Note: we could get a better transaction size if the full transaction was passed down to libpreflight
        transaction_size_bytes: estimate_max_transaction_size_for_operation(
            &OperationBody::InvokeHostFunction(op.clone()),
            &soroban_resources.footprint,
        )
        .context("cannot estimate maximum transaction size")?,
        contract_events_size_bytes: final_contract_events_size,
    };
    let rent_changes = extract_rent_changes(&ledger_changes);

    finalize_transaction_data_and_min_fee(
        pre_storage,
        &transaction_resources,
        soroban_resources,
        &rent_changes,
        current_ledger_seq,
        bucket_list_size,
    )
}

fn estimate_max_transaction_size_for_operation(
    op: &OperationBody,
    fp: &LedgerFootprint,
) -> Result<u32> {
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
    ledger_changes: &Vec<LedgerEntryChange>,
    footprint: &Footprint,
    budget: &Budget,
) -> Result<SorobanResources> {
    let ledger_footprint = storage_footprint_to_ledger_footprint(footprint)
        .context("cannot convert storage footprint to ledger footprint")?;
    let read_bytes: u32 = ledger_changes
        .iter()
        .map(|c| c.encoded_key.len() as u32 + c.old_entry_size_bytes)
        .sum();

    let write_bytes: u32 = ledger_changes
        .iter()
        .map(|c| {
            c.encoded_key.len() as u32 + c.encoded_new_value.as_ref().map_or(0, Vec::len) as u32
        })
        .sum();

    // Add a 20% leeway with a minimum of 50k instructions
    let budget_instructions = budget
        .get_cpu_insns_consumed()
        .context("cannot get instructions consumed")?;
    let instructions = max(budget_instructions + 50000, budget_instructions * 120 / 100);
    Ok(SorobanResources {
        footprint: ledger_footprint,
        instructions: u32::try_from(instructions)?,
        read_bytes,
        write_bytes,
    })
}

fn get_fee_configurations(
    ledger_storage: &LedgerStorage,
    bucket_list_size: u64,
) -> Result<(FeeConfiguration, RentFeeConfiguration)> {
    let ConfigSettingEntry::ContractComputeV0(compute) =
        ledger_storage.get_configuration_setting(ConfigSettingId::ContractComputeV0)?
    else {
        bail!("unexpected config setting entry for ComputeV0 key");
    };

    let ConfigSettingEntry::ContractLedgerCostV0(ledger_cost) =
        ledger_storage.get_configuration_setting(ConfigSettingId::ContractLedgerCostV0)?
    else {
        bail!("unexpected config setting entry for LedgerCostV0 key");
    };

    let ConfigSettingEntry::ContractHistoricalDataV0(historical_data) =
        ledger_storage.get_configuration_setting(ConfigSettingId::ContractHistoricalDataV0)?
    else {
        bail!("unexpected config setting entry for HistoricalDataV0 key");
    };

    let ConfigSettingEntry::ContractEventsV0(events) =
        ledger_storage.get_configuration_setting(ConfigSettingId::ContractEventsV0)?
    else {
        bail!("unexpected config setting entry for EventsV0 key");
    };

    let ConfigSettingEntry::ContractBandwidthV0(bandwidth) =
        ledger_storage.get_configuration_setting(ConfigSettingId::ContractBandwidthV0)?
    else {
        bail!("unexpected config setting entry for BandwidthV0 key");
    };

    let ConfigSettingEntry::StateExpiration(state_expiration) =
        ledger_storage.get_configuration_setting(ConfigSettingId::StateExpiration)?
    else {
        bail!("unexpected config setting entry for StateExpiration key");
    };

    let write_fee_configuration = WriteFeeConfiguration {
        bucket_list_target_size_bytes: ledger_cost.bucket_list_target_size_bytes,
        write_fee_1kb_bucket_list_low: ledger_cost.write_fee1_kb_bucket_list_low,
        write_fee_1kb_bucket_list_high: ledger_cost.write_fee1_kb_bucket_list_high,
        bucket_list_write_fee_growth_factor: ledger_cost.bucket_list_write_fee_growth_factor,
    };

    let write_fee_per_1kb =
        compute_write_fee_per_1kb(bucket_list_size as i64, &write_fee_configuration);

    let fee_configuration = FeeConfiguration {
        fee_per_instruction_increment: compute.fee_rate_per_instructions_increment,
        fee_per_read_entry: ledger_cost.fee_read_ledger_entry,
        fee_per_write_entry: ledger_cost.fee_write_ledger_entry,
        fee_per_read_1kb: ledger_cost.fee_read1_kb,
        fee_per_write_1kb: write_fee_per_1kb,
        fee_per_historical_1kb: historical_data.fee_historical1_kb,
        fee_per_contract_event_1kb: events.fee_contract_events1_kb,
        fee_per_transaction_size_1kb: bandwidth.fee_tx_size1_kb,
    };
    let rent_fee_configuration = RentFeeConfiguration {
        fee_per_write_1kb: write_fee_per_1kb,
        fee_per_write_entry: ledger_cost.fee_write_ledger_entry,
        persistent_rent_rate_denominator: state_expiration.persistent_rent_rate_denominator,
        temporary_rent_rate_denominator: state_expiration.temp_rent_rate_denominator,
    };
    Ok((fee_configuration, rent_fee_configuration))
}

fn calculate_unmodified_ledger_entry_bytes(
    ledger_entries: &Vec<LedgerKey>,
    pre_storage: &LedgerStorage,
    include_expired: bool,
) -> Result<u32> {
    let mut res: usize = 0;
    for lk in ledger_entries {
        let key_xdr = lk
            .to_xdr()
            .with_context(|| format!("cannot marshall ledger key {lk:?}"))?;
        let key_size = key_xdr.len();
        let entry_xdr = pre_storage
            .get_xdr(lk, include_expired)
            .with_context(|| format!("cannot get xdr of ledger entry with key {lk:?}"))?;
        let entry_size = entry_xdr.len();
        res += key_size + entry_size;
    }
    Ok(res as u32)
}

fn calculate_contract_events_size_bytes(events: &Vec<DiagnosticEvent>) -> Result<u32> {
    let mut res: u32 = 0;
    for e in events {
        if e.event.type_ != ContractEventType::Contract {
            continue;
        }
        let event_xdr = e
            .to_xdr()
            .with_context(|| format!("cannot marshal event {e:?}"))?;
        res += u32::try_from(event_xdr.len())?;
    }
    Ok(res)
}

fn storage_footprint_to_ledger_footprint(foot: &Footprint) -> Result<LedgerFootprint, xdr::Error> {
    let mut read_only: Vec<LedgerKey> = Vec::with_capacity(foot.0.len());
    let mut read_write: Vec<LedgerKey> = Vec::with_capacity(foot.0.len());
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

fn finalize_transaction_data_and_min_fee(
    pre_storage: &LedgerStorage,
    transaction_resources: &TransactionResources,
    soroban_resources: SorobanResources,
    rent_changes: &Vec<LedgerEntryRentChange>,
    current_ledger_seq: u32,
    bucket_list_size: u64,
) -> Result<(SorobanTransactionData, i64)> {
    let (fee_configuration, rent_fee_configuration) =
        get_fee_configurations(pre_storage, bucket_list_size)
            .context("failed to obtain configuration settings from the network")?;
    let (non_refundable_fee, refundable_fee) =
        compute_transaction_resource_fee(transaction_resources, &fee_configuration);
    let rent_fee = compute_rent_fee(&rent_changes, &rent_fee_configuration, current_ledger_seq);
    let transaction_data = SorobanTransactionData {
        resources: soroban_resources,
        refundable_fee: refundable_fee + rent_fee,
        ext: ExtensionPoint::V0,
    };
    let res = (
        transaction_data,
        refundable_fee + non_refundable_fee + rent_fee,
    );
    Ok(res)
}

pub(crate) fn compute_bump_footprint_exp_transaction_data_and_min_fee(
    footprint: LedgerFootprint,
    ledgers_to_expire: u32,
    ledger_storage: &LedgerStorage,
    bucket_list_size: u64,
    current_ledger_seq: u32,
) -> Result<(SorobanTransactionData, i64)> {
    let rent_changes = compute_bump_footprint_rent_changes(
        &footprint,
        ledger_storage,
        ledgers_to_expire,
        current_ledger_seq,
    )
    .context("cannot compute bump rent changes")?;
    let read_bytes = calculate_unmodified_ledger_entry_bytes(
        footprint.read_only.as_vec(),
        ledger_storage,
        false,
    )
    .context("cannot calculate read_bytes resource")?;
    let soroban_resources = SorobanResources {
        footprint,
        instructions: 0,
        read_bytes,
        write_bytes: 0,
    };
    let transaction_size_bytes = estimate_max_transaction_size_for_operation(
        &OperationBody::BumpFootprintExpiration(BumpFootprintExpirationOp {
            ext: ExtensionPoint::V0,
            ledgers_to_expire,
        }),
        &soroban_resources.footprint,
    )
    .context("cannot estimate maximum transaction size")?;
    let transaction_resources = TransactionResources {
        instructions: 0,
        read_entries: u32::try_from(soroban_resources.footprint.read_only.as_vec().len())?,
        write_entries: 0,
        read_bytes: soroban_resources.read_bytes,
        write_bytes: 0,
        transaction_size_bytes,
        contract_events_size_bytes: 0,
    };
    finalize_transaction_data_and_min_fee(
        &ledger_storage,
        &transaction_resources,
        soroban_resources,
        &rent_changes,
        current_ledger_seq,
        bucket_list_size,
    )
}

fn compute_bump_footprint_rent_changes(
    footprint: &LedgerFootprint,
    ledger_storage: &LedgerStorage,
    ledgers_to_expire: u32,
    current_ledger_seq: u32,
) -> Result<Vec<LedgerEntryRentChange>> {
    let mut rent_changes: Vec<LedgerEntryRentChange> =
        Vec::with_capacity(footprint.read_only.len());
    for key in (&footprint).read_only.as_vec() {
        let unmodified_entry_and_expiration = ledger_storage
            .get(key, false)
            .with_context(|| format!("cannot find bump footprint ledger entry with key {key:?}"))?;
        let size = (key.to_xdr()?.len() + unmodified_entry_and_expiration.0.to_xdr()?.len()) as u32;
        let expirable_entry: Box<dyn ExpirableLedgerEntry> = (&unmodified_entry_and_expiration)
            .try_into()
            .map_err(|e: String| {
                Error::msg(e.clone()).context("incorrect ledger entry type in footprint")
            })?;
        let new_expiration_ledger = current_ledger_seq + ledgers_to_expire;
        if new_expiration_ledger <= expirable_entry.expiration_ledger_seq() {
            // The bump would be ineffective
            continue;
        }
        let rent_change = LedgerEntryRentChange {
            is_persistent: expirable_entry.durability() == Persistent,
            old_size_bytes: size,
            new_size_bytes: size,
            old_expiration_ledger: expirable_entry.expiration_ledger_seq(),
            new_expiration_ledger,
        };
        rent_changes.push(rent_change);
    }
    Ok(rent_changes)
}

pub(crate) fn compute_restore_footprint_transaction_data_and_min_fee(
    footprint: LedgerFootprint,
    ledger_storage: &LedgerStorage,
    bucket_list_size: u64,
    current_ledger_seq: u32,
) -> Result<(SorobanTransactionData, i64)> {
    let ConfigSettingEntry::StateExpiration(state_expiration) =
        ledger_storage.get_configuration_setting(ConfigSettingId::StateExpiration)?
    else {
        bail!("unexpected config setting entry for StateExpiration key");
    };
    let rent_changes = compute_restore_footprint_rent_changes(
        &footprint,
        ledger_storage,
        state_expiration.min_persistent_entry_expiration,
        current_ledger_seq,
    )
    .context("cannot compute restore rent changes")?;
    let write_bytes = calculate_unmodified_ledger_entry_bytes(
        footprint.read_write.as_vec(),
        ledger_storage,
        true,
    )
    .context("cannot calculate write_bytes resource")?;
    let soroban_resources = SorobanResources {
        footprint,
        instructions: 0,
        read_bytes: write_bytes,
        write_bytes,
    };
    let entry_count = u32::try_from(soroban_resources.footprint.read_write.as_vec().len())?;
    let transaction_size_bytes = estimate_max_transaction_size_for_operation(
        &OperationBody::RestoreFootprint(RestoreFootprintOp {
            ext: ExtensionPoint::V0,
        }),
        &soroban_resources.footprint,
    )
    .context("cannot estimate maximum transaction size")?;
    let transaction_resources = TransactionResources {
        instructions: 0,
        read_entries: entry_count,
        write_entries: entry_count,
        read_bytes: soroban_resources.read_bytes,
        write_bytes: soroban_resources.write_bytes,
        transaction_size_bytes,
        contract_events_size_bytes: 0,
    };
    finalize_transaction_data_and_min_fee(
        ledger_storage,
        &transaction_resources,
        soroban_resources,
        &rent_changes,
        current_ledger_seq,
        bucket_list_size,
    )
}

fn compute_restore_footprint_rent_changes(
    footprint: &LedgerFootprint,
    ledger_storage: &LedgerStorage,
    min_persistent_entry_expiration: u32,
    current_ledger_seq: u32,
) -> Result<Vec<LedgerEntryRentChange>> {
    let mut rent_changes: Vec<LedgerEntryRentChange> =
        Vec::with_capacity(footprint.read_write.len());
    for key in footprint.read_write.as_vec() {
        let unmodified_entry_and_expiration = ledger_storage.get(key, true).with_context(|| {
            format!("cannot find restore footprint ledger entry with key {key:?}")
        })?;
        let size = (key.to_xdr()?.len() + unmodified_entry_and_expiration.0.to_xdr()?.len()) as u32;
        let expirable_entry: Box<dyn ExpirableLedgerEntry> = (&unmodified_entry_and_expiration)
            .try_into()
            .map_err(|e: String| {
                Error::msg(e.clone()).context("incorrect ledger entry type in footprint")
            })?;
        ensure!(
            expirable_entry.durability() == Persistent,
            "non-persistent entry in footprint: key = {key:?}"
        );
        if !expirable_entry.has_expired(current_ledger_seq) {
            // noop (the entry hadn't expired)
            continue;
        }
        let new_expiration_ledger =
            get_restored_ledger_sequence(current_ledger_seq, min_persistent_entry_expiration);
        let rent_change = LedgerEntryRentChange {
            is_persistent: true,
            old_size_bytes: 0,
            new_size_bytes: size,
            old_expiration_ledger: 0,
            new_expiration_ledger,
        };
        rent_changes.push(rent_change);
    }
    Ok(rent_changes)
}
