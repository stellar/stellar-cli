use anyhow::{bail, ensure, Context, Error, Result};
use ledger_storage::LedgerStorage;
use soroban_env_host::budget::Budget;
use soroban_env_host::e2e_invoke::{
    extract_rent_changes, get_ledger_changes, LedgerEntryChange, TtlEntryMap,
};
use soroban_env_host::fees::{
    compute_rent_fee, compute_transaction_resource_fee, compute_write_fee_per_1kb,
    FeeConfiguration, LedgerEntryRentChange, RentFeeConfiguration, TransactionResources,
    WriteFeeConfiguration,
};
use soroban_env_host::storage::{AccessType, Footprint, Storage};
use soroban_env_host::xdr;
use soroban_env_host::xdr::ContractDataDurability::Persistent;
use soroban_env_host::xdr::{
    ConfigSettingEntry, ConfigSettingId, ContractEventType, DecoratedSignature, DiagnosticEvent,
    ExtendFootprintTtlOp, ExtensionPoint, InvokeHostFunctionOp, LedgerFootprint, LedgerKey, Limits,
    Memo, MuxedAccount, MuxedAccountMed25519, Operation, OperationBody, Preconditions,
    RestoreFootprintOp, ScVal, SequenceNumber, Signature, SignatureHint, SorobanResources,
    SorobanTransactionData, Transaction, TransactionExt, TransactionV1Envelope, Uint256, WriteXdr,
};
use state_ttl::{get_restored_ledger_sequence, TTLLedgerEntry};
use std::cmp::max;
use std::convert::{TryFrom, TryInto};

use crate::CResourceConfig;

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_host_function_transaction_data_and_min_fee(
    op: &InvokeHostFunctionOp,
    pre_storage: &LedgerStorage,
    post_storage: &Storage,
    budget: &Budget,
    resource_config: CResourceConfig,
    events: &[DiagnosticEvent],
    invocation_result: &ScVal,
    bucket_list_size: u64,
    current_ledger_seq: u32,
) -> Result<(SorobanTransactionData, i64)> {
    let ledger_changes = get_ledger_changes(budget, post_storage, pre_storage, TtlEntryMap::new())?;
    let soroban_resources = calculate_host_function_soroban_resources(
        &ledger_changes,
        &post_storage.footprint,
        budget,
        resource_config,
    )
    .context("cannot compute host function resources")?;

    let contract_events_size =
        calculate_contract_events_size_bytes(events).context("cannot calculate events size")?;
    let invocation_return_size = u32::try_from(invocation_result.to_xdr(Limits::none())?.len())?;
    // This is totally unintuitive, but it's what's expected by the library
    let final_contract_events_size = contract_events_size + invocation_return_size;

    let transaction_resources = TransactionResources {
        instructions: soroban_resources.instructions,
        read_entries: u32::try_from(soroban_resources.footprint.read_only.as_vec().len())?,
        write_entries: u32::try_from(soroban_resources.footprint.read_write.as_vec().len())?,
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
                resource_fee: 0,
                ext: ExtensionPoint::V0,
            }),
        },
        signatures: signatures.try_into()?,
    };

    let envelope_xdr = envelope.to_xdr(Limits::none())?;
    let envelope_size = envelope_xdr.len();

    // Add a 15% leeway
    let envelope_size = envelope_size * 115 / 100;
    Ok(u32::try_from(envelope_size)?)
}

#[allow(clippy::cast_possible_truncation)]
fn calculate_host_function_soroban_resources(
    ledger_changes: &[LedgerEntryChange],
    footprint: &Footprint,
    budget: &Budget,
    resource_config: CResourceConfig,
) -> Result<SorobanResources> {
    let ledger_footprint = storage_footprint_to_ledger_footprint(footprint)
        .context("cannot convert storage footprint to ledger footprint")?;
    let read_bytes: u32 = ledger_changes.iter().map(|c| c.old_entry_size_bytes).sum();

    let write_bytes: u32 = ledger_changes
        .iter()
        .map(|c| c.encoded_new_value.as_ref().map_or(0, Vec::len) as u32)
        .sum();

    // Add a 20% leeway with a minimum of 3 million instructions
    let budget_instructions = budget
        .get_cpu_insns_consumed()
        .context("cannot get instructions consumed")?;
    let instructions = max(
        budget_instructions + resource_config.instruction_leeway,
        budget_instructions * 120 / 100,
    );
    Ok(SorobanResources {
        footprint: ledger_footprint,
        instructions: u32::try_from(instructions)?,
        read_bytes,
        write_bytes,
    })
}

#[allow(clippy::cast_possible_wrap)]
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

    let ConfigSettingEntry::StateArchival(state_archival) =
        ledger_storage.get_configuration_setting(ConfigSettingId::StateArchival)?
    else {
        bail!("unexpected config setting entry for StateArchival key");
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
        persistent_rent_rate_denominator: state_archival.persistent_rent_rate_denominator,
        temporary_rent_rate_denominator: state_archival.temp_rent_rate_denominator,
    };
    Ok((fee_configuration, rent_fee_configuration))
}

#[allow(clippy::cast_possible_truncation)]
fn calculate_unmodified_ledger_entry_bytes(
    ledger_entries: &[LedgerKey],
    pre_storage: &LedgerStorage,
    include_not_live: bool,
) -> Result<u32> {
    let mut res: usize = 0;
    for lk in ledger_entries {
        let entry_xdr = pre_storage
            .get_xdr(lk, include_not_live)
            .with_context(|| format!("cannot get xdr of ledger entry with key {lk:?}"))?;
        let entry_size = entry_xdr.len();
        res += entry_size;
    }
    Ok(res as u32)
}

fn calculate_contract_events_size_bytes(events: &[DiagnosticEvent]) -> Result<u32> {
    let mut res: u32 = 0;
    for e in events {
        if e.event.type_ != ContractEventType::Contract
            && e.event.type_ != ContractEventType::System
        {
            continue;
        }
        let event_xdr = e
            .to_xdr(Limits::none())
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
    let rent_fee = compute_rent_fee(rent_changes, &rent_fee_configuration, current_ledger_seq);
    let resource_fee = refundable_fee + non_refundable_fee + rent_fee;
    let transaction_data = SorobanTransactionData {
        resources: soroban_resources,
        resource_fee,
        ext: ExtensionPoint::V0,
    };
    let res = (transaction_data, resource_fee);
    Ok(res)
}

pub(crate) fn compute_extend_footprint_ttl_transaction_data_and_min_fee(
    footprint: LedgerFootprint,
    extend_to: u32,
    ledger_storage: &LedgerStorage,
    bucket_list_size: u64,
    current_ledger_seq: u32,
) -> Result<(SorobanTransactionData, i64)> {
    let rent_changes = compute_extend_footprint_rent_changes(
        &footprint,
        ledger_storage,
        extend_to,
        current_ledger_seq,
    )
    .context("cannot compute extend rent changes")?;

    let unmodified_entry_bytes = calculate_unmodified_ledger_entry_bytes(
        footprint.read_only.as_slice(),
        ledger_storage,
        false,
    )
    .context("cannot calculate read_bytes resource")?;

    let soroban_resources = SorobanResources {
        footprint,
        instructions: 0,
        read_bytes: unmodified_entry_bytes,
        write_bytes: 0,
    };
    let transaction_size_bytes = estimate_max_transaction_size_for_operation(
        &OperationBody::ExtendFootprintTtl(ExtendFootprintTtlOp {
            ext: ExtensionPoint::V0,
            extend_to,
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
        ledger_storage,
        &transaction_resources,
        soroban_resources,
        &rent_changes,
        current_ledger_seq,
        bucket_list_size,
    )
}

#[allow(clippy::cast_possible_truncation)]
fn compute_extend_footprint_rent_changes(
    footprint: &LedgerFootprint,
    ledger_storage: &LedgerStorage,
    extend_to: u32,
    current_ledger_seq: u32,
) -> Result<Vec<LedgerEntryRentChange>> {
    let mut rent_changes: Vec<LedgerEntryRentChange> =
        Vec::with_capacity(footprint.read_only.len());
    for key in footprint.read_only.as_slice() {
        let unmodified_entry_and_ttl = ledger_storage.get(key, false).with_context(|| {
            format!("cannot find extend footprint ledger entry with key {key:?}")
        })?;
        let size = (key.to_xdr(Limits::none())?.len()
            + unmodified_entry_and_ttl.0.to_xdr(Limits::none())?.len()) as u32;
        let ttl_entry: Box<dyn TTLLedgerEntry> =
            (&unmodified_entry_and_ttl)
                .try_into()
                .map_err(|e: String| {
                    Error::msg(e.clone()).context("incorrect ledger entry type in footprint")
                })?;
        let new_live_until_ledger = current_ledger_seq + extend_to;
        if new_live_until_ledger <= ttl_entry.live_until_ledger_seq() {
            // The extend would be ineffective
            continue;
        }
        let rent_change = LedgerEntryRentChange {
            is_persistent: ttl_entry.durability() == Persistent,
            old_size_bytes: size,
            new_size_bytes: size,
            old_live_until_ledger: ttl_entry.live_until_ledger_seq(),
            new_live_until_ledger,
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
    let ConfigSettingEntry::StateArchival(state_archival) =
        ledger_storage.get_configuration_setting(ConfigSettingId::StateArchival)?
    else {
        bail!("unexpected config setting entry for StateArchival key");
    };
    let rent_changes = compute_restore_footprint_rent_changes(
        &footprint,
        ledger_storage,
        state_archival.min_persistent_ttl,
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
    let transaction_size_bytes = estimate_max_transaction_size_for_operation(
        &OperationBody::RestoreFootprint(RestoreFootprintOp {
            ext: ExtensionPoint::V0,
        }),
        &soroban_resources.footprint,
    )
    .context("cannot estimate maximum transaction size")?;
    let transaction_resources = TransactionResources {
        instructions: 0,
        read_entries: 0,
        write_entries: u32::try_from(soroban_resources.footprint.read_write.as_vec().len())?,
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

#[allow(clippy::cast_possible_truncation)]
fn compute_restore_footprint_rent_changes(
    footprint: &LedgerFootprint,
    ledger_storage: &LedgerStorage,
    min_persistent_ttl: u32,
    current_ledger_seq: u32,
) -> Result<Vec<LedgerEntryRentChange>> {
    let mut rent_changes: Vec<LedgerEntryRentChange> =
        Vec::with_capacity(footprint.read_write.len());
    for key in footprint.read_write.as_vec() {
        let unmodified_entry_and_ttl = ledger_storage.get(key, true).with_context(|| {
            format!("cannot find restore footprint ledger entry with key {key:?}")
        })?;
        let size = (key.to_xdr(Limits::none())?.len()
            + unmodified_entry_and_ttl.0.to_xdr(Limits::none())?.len()) as u32;
        let ttl_entry: Box<dyn TTLLedgerEntry> =
            (&unmodified_entry_and_ttl)
                .try_into()
                .map_err(|e: String| {
                    Error::msg(e.clone()).context("incorrect ledger entry type in footprint")
                })?;
        ensure!(
            ttl_entry.durability() == Persistent,
            "non-persistent entry in footprint: key = {key:?}"
        );
        if ttl_entry.is_live(current_ledger_seq) {
            // noop (the entry is alive)
            continue;
        }
        let new_live_until_ledger =
            get_restored_ledger_sequence(current_ledger_seq, min_persistent_ttl);
        let rent_change = LedgerEntryRentChange {
            is_persistent: true,
            old_size_bytes: 0,
            new_size_bytes: size,
            old_live_until_ledger: 0,
            new_live_until_ledger,
        };
        rent_changes.push(rent_change);
    }
    Ok(rent_changes)
}
