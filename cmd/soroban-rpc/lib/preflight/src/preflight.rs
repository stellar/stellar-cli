use anyhow::{anyhow, bail, Result};
use fees;
use ledger_storage::LedgerStorage;
use soroban_env_host::auth::RecordedAuthPayload;
use soroban_env_host::budget::Budget;
use soroban_env_host::events::Events;
use soroban_env_host::storage::Storage;
use soroban_env_host::xdr::{
    AccountId, ConfigSettingEntry, ConfigSettingId, DiagnosticEvent, InvokeHostFunctionOp,
    LedgerFootprint, LedgerKey, OperationBody, ScVal, SorobanAddressCredentials,
    SorobanAuthorizationEntry, SorobanCredentials, SorobanTransactionData, VecM,
};
use soroban_env_host::{DiagnosticLevel, Host, LedgerInfo};
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use std::rc::Rc;

pub(crate) struct RestorePreamble {
    pub(crate) transaction_data: SorobanTransactionData,
    pub(crate) min_fee: i64,
}

pub(crate) struct PreflightResult {
    pub(crate) auth: Vec<SorobanAuthorizationEntry>,
    pub(crate) result: Option<ScVal>,
    pub(crate) transaction_data: SorobanTransactionData,
    pub(crate) min_fee: i64,
    pub(crate) events: Vec<DiagnosticEvent>,
    pub(crate) cpu_instructions: u64,
    pub(crate) memory_bytes: u64,
    pub(crate) restore_preamble: Option<RestorePreamble>,
}

pub(crate) fn preflight_invoke_hf_op(
    ledger_storage: LedgerStorage,
    bucket_list_size: u64,
    invoke_hf_op: InvokeHostFunctionOp,
    source_account: AccountId,
    ledger_info: LedgerInfo,
) -> Result<PreflightResult> {
    let ledger_storage_rc = Rc::new(ledger_storage);
    let budget = get_budget_from_network_config_params(&ledger_storage_rc)?;
    let storage = Storage::with_recording_footprint(ledger_storage_rc.clone());
    let host = Host::with_storage_and_budget(storage, budget);

    // We make an assumption here:
    // - if a transaction doesn't include any soroban authorization entries the client either
    // doesn't know the authorization entries, or there are none. In either case it is best to
    // record the authorization entries and return them to the client.
    // - if a transaction *does* include soroban authorization entries, then the client *already*
    // knows the needed entries, so we should try them in enforcing mode so that we can validate
    // them, and return the correct fees and footprint.
    let needs_auth_recording = invoke_hf_op.auth.is_empty();
    if needs_auth_recording {
        host.switch_to_recording_auth()?;
    } else {
        host.set_authorization_entries(invoke_hf_op.auth.to_vec())?;
    }

    host.set_diagnostic_level(DiagnosticLevel::Debug)?;
    host.set_source_account(source_account.clone())?;
    host.set_ledger_info(ledger_info.clone())?;

    // Run the preflight.
    let result = host.invoke_function(invoke_hf_op.host_function.clone())?;
    let auths: VecM<SorobanAuthorizationEntry> = if needs_auth_recording {
        let payloads = host.get_recorded_auth_payloads()?;
        VecM::try_from(
            payloads
                .iter()
                .map(recorded_auth_payload_to_xdr)
                .collect::<Vec<_>>(),
        )?
    } else {
        invoke_hf_op.auth
    };

    let budget = host.budget_cloned();
    // Recover, convert and return the storage footprint and other values to C.
    let (storage, events) = host.try_finish()?;

    let diagnostic_events = host_events_to_diagnostic_events(&events);
    let (transaction_data, min_fee) = fees::compute_host_function_transaction_data_and_min_fee(
        &InvokeHostFunctionOp {
            host_function: invoke_hf_op.host_function,
            auth: auths.clone(),
        },
        &ledger_storage_rc,
        &storage,
        &budget,
        &diagnostic_events,
        bucket_list_size,
        ledger_info.sequence_number,
    )?;

    let entries = ledger_storage_rc.get_ledger_keys_requiring_restore();
    let restore_preamble = if entries.len() > 0 {
        let read_write_vec: Vec<LedgerKey> = Vec::from_iter(entries);
        let restore_footprint = LedgerFootprint {
            read_only: VecM::default(),
            read_write: read_write_vec.try_into()?,
        };
        let (transaction_data, min_fee) =
            fees::compute_restore_footprint_transaction_data_and_min_fee(
                restore_footprint,
                &ledger_storage_rc,
                bucket_list_size,
                ledger_info.sequence_number,
            )?;
        Some(RestorePreamble {
            transaction_data,
            min_fee,
        })
    } else {
        None
    };

    Ok(PreflightResult {
        auth: auths.to_vec(),
        result: Some(result),
        transaction_data,
        min_fee,
        events: diagnostic_events,
        cpu_instructions: budget.get_cpu_insns_consumed()?,
        memory_bytes: budget.get_mem_bytes_consumed()?,
        restore_preamble,
    })
}

fn recorded_auth_payload_to_xdr(payload: &RecordedAuthPayload) -> SorobanAuthorizationEntry {
    match (payload.address.clone(), payload.nonce) {
        (Some(address), Some(nonce)) => SorobanAuthorizationEntry {
            credentials: SorobanCredentials::Address(SorobanAddressCredentials {
                address,
                nonce,
                // signature is left empty. This is where the client will put their signatures when
                // submitting the transaction.
                signature_expiration_ledger: 0,
                signature: ScVal::Void,
            }),
            root_invocation: payload.invocation.clone(),
        },
        (None, None) => SorobanAuthorizationEntry {
            credentials: SorobanCredentials::SourceAccount,
            root_invocation: payload.invocation.clone(),
        },
        // the address and the nonce can't be present independently
        (a,n) =>
            panic!("recorded_auth_payload_to_xdr: address and nonce present independently (address: {:?}, nonce: {:?})", a, n),
    }
}

fn host_events_to_diagnostic_events(events: &Events) -> Vec<DiagnosticEvent> {
    let mut res: Vec<DiagnosticEvent> = Vec::new();
    for e in &events.0 {
        let diagnostic_event = DiagnosticEvent {
            in_successful_contract_call: !e.failed_call,
            event: e.event.clone(),
        };
        res.push(diagnostic_event);
    }
    res
}

fn get_budget_from_network_config_params(ledger_storage: &LedgerStorage) -> Result<Budget> {
    let ConfigSettingEntry::ContractComputeV0(compute) =
        ledger_storage.get_configuration_setting(ConfigSettingId::ContractComputeV0)?
        else {
            bail!("get_budget_from_network_config_params((): unexpected config setting entry for ComputeV0 key");
        };

    let ConfigSettingEntry::ContractCostParamsCpuInstructions(cost_params_cpu) = ledger_storage
        .get_configuration_setting(ConfigSettingId::ContractCostParamsCpuInstructions)?
        else {
            bail!("get_budget_from_network_config_params((): unexpected config setting entry for ComputeV0 key");
        };

    let ConfigSettingEntry::ContractCostParamsMemoryBytes(cost_params_memory) =
        ledger_storage.get_configuration_setting(ConfigSettingId::ContractCostParamsMemoryBytes)?
        else {
            bail!("get_budget_from_network_config_params((): unexpected config setting entry for ComputeV0 key");
        };

    let budget = Budget::try_from_configs(
        compute.tx_max_instructions as u64,
        compute.tx_memory_limit as u64,
        cost_params_cpu,
        cost_params_memory,
    )?;
    Ok(budget)
}

pub(crate) fn preflight_footprint_expiration_op(
    ledger_storage: &LedgerStorage,
    bucket_list_size: u64,
    op_body: OperationBody,
    footprint: LedgerFootprint,
    current_ledger_seq: u32,
) -> Result<PreflightResult> {
    match op_body {
        OperationBody::BumpFootprintExpiration(op) => preflight_bump_footprint_expiration(
            footprint,
            op.ledgers_to_expire,
            ledger_storage,
            bucket_list_size,
            current_ledger_seq,
        ),
        OperationBody::RestoreFootprint(_) => preflight_restore_footprint(
            footprint,
            ledger_storage,
            bucket_list_size,
            current_ledger_seq,
        ),
        op => Err(anyhow!(
            "preflight_footprint_expiration_op(): unsupported operation type {}",
            op.name()
        )),
    }
}

fn preflight_bump_footprint_expiration(
    footprint: LedgerFootprint,
    ledgers_to_expire: u32,
    ledger_storage: &LedgerStorage,
    bucket_list_size: u64,
    current_ledger_seq: u32,
) -> Result<PreflightResult> {
    let (transaction_data, min_fee) =
        fees::compute_bump_footprint_exp_transaction_data_and_min_fee(
            footprint,
            ledgers_to_expire,
            ledger_storage,
            bucket_list_size,
            current_ledger_seq,
        )?;
    Ok(PreflightResult {
        auth: vec![],
        result: None,
        transaction_data,
        min_fee,
        events: vec![],
        cpu_instructions: 0,
        memory_bytes: 0,
        restore_preamble: None,
    })
}

fn preflight_restore_footprint(
    footprint: LedgerFootprint,
    ledger_storage: &LedgerStorage,
    bucket_list_size: u64,
    current_ledger_seq: u32,
) -> Result<PreflightResult> {
    let (transaction_data, min_fee) = fees::compute_restore_footprint_transaction_data_and_min_fee(
        footprint,
        ledger_storage,
        bucket_list_size,
        current_ledger_seq,
    )?;
    Ok(PreflightResult {
        auth: vec![],
        result: None,
        transaction_data,
        min_fee,
        events: vec![],
        cpu_instructions: 0,
        memory_bytes: 0,
        restore_preamble: None,
    })
}
