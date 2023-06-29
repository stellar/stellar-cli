mod fees;
mod ledger_storage;

extern crate base64;
extern crate libc;
extern crate sha2;
extern crate soroban_env_host;

use ledger_storage::LedgerStorage;
use sha2::{Digest, Sha256};
use soroban_env_host::auth::RecordedAuthPayload;
use soroban_env_host::budget::Budget;
use soroban_env_host::events::Events;
use soroban_env_host::storage::Storage;
use soroban_env_host::xdr::{
    AccountId, DiagnosticEvent, InvokeHostFunctionOp, LedgerFootprint, OperationBody, ReadXdr,
    ScVec, SorobanAddressCredentials, SorobanAuthorizationEntry, SorobanCredentials, WriteXdr,
};
use soroban_env_host::{DiagnosticLevel, Host, LedgerInfo};
use std::error::Error;
use std::ffi::{CStr, CString};
use std::panic;
use std::ptr::null_mut;
use std::rc::Rc;
use std::{error, mem};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CLedgerInfo {
    pub protocol_version: u32,
    pub sequence_number: u32,
    pub timestamp: u64,
    pub network_passphrase: *const libc::c_char,
    pub base_reserve: u32,
    pub min_temp_entry_expiration: u32,
    pub min_persistent_entry_expiration: u32,
    pub max_entry_expiration: u32,
}

impl From<CLedgerInfo> for LedgerInfo {
    fn from(c: CLedgerInfo) -> Self {
        let network_passphrase_cstr = unsafe { CStr::from_ptr(c.network_passphrase) };
        Self {
            protocol_version: c.protocol_version,
            sequence_number: c.sequence_number,
            timestamp: c.timestamp,
            network_id: Sha256::digest(network_passphrase_cstr.to_str().unwrap().as_bytes()).into(),
            base_reserve: c.base_reserve,
            min_temp_entry_expiration: c.min_temp_entry_expiration,
            min_persistent_entry_expiration: c.min_persistent_entry_expiration,
            max_entry_expiration: c.max_entry_expiration,
        }
    }
}

#[repr(C)]
pub struct CPreflightResult {
    pub error: *mut libc::c_char, // Error string in case of error, otherwise null
    pub auth: *mut *mut libc::c_char, // NULL terminated array of XDR SorobanAuthorizationEntrys in base64
    pub result: *mut libc::c_char,    // XDR SCVal in base64
    pub transaction_data: *mut libc::c_char, // SorobanTransactionData XDR in base64
    pub min_fee: i64,                 // Minimum recommended resource fee
    pub events: *mut *mut libc::c_char, // NULL terminated array of XDR ContractEvents in base64
    pub cpu_instructions: u64,
    pub memory_bytes: u64,
}

fn preflight_error(str: String) -> *mut CPreflightResult {
    let c_str = CString::new(str).unwrap();
    // transfer ownership to caller
    // caller needs to invoke free_preflight_result(result) when done
    Box::into_raw(Box::new(CPreflightResult {
        error: c_str.into_raw(),
        auth: null_mut(),
        result: null_mut(),
        transaction_data: null_mut(),
        min_fee: 0,
        events: null_mut(),
        cpu_instructions: 0,
        memory_bytes: 0,
    }))
}

#[no_mangle]
pub extern "C" fn preflight_invoke_hf_op(
    handle: libc::uintptr_t, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHasconst
    invoke_hf_op: *const libc::c_char, // InvokeHostFunctionOp XDR in base64
    source_account: *const libc::c_char, // AccountId XDR in base64
    ledger_info: CLedgerInfo,
) -> *mut CPreflightResult {
    catch_preflight_panic(Box::new(move || {
        preflight_invoke_hf_op_or_maybe_panic(handle, invoke_hf_op, source_account, ledger_info)
    }))
}

fn preflight_invoke_hf_op_or_maybe_panic(
    handle: libc::uintptr_t, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHas
    invoke_hf_op: *const libc::c_char, // InvokeHostFunctionOp XDR in base64
    source_account: *const libc::c_char, // AccountId XDR in base64
    ledger_info: CLedgerInfo,
) -> Result<CPreflightResult, Box<dyn error::Error>> {
    let invoke_hf_op_cstr = unsafe { CStr::from_ptr(invoke_hf_op) };
    let invoke_hf_op = InvokeHostFunctionOp::from_xdr_base64(invoke_hf_op_cstr.to_str()?)?;
    let source_account_cstr = unsafe { CStr::from_ptr(source_account) };
    let source_account = AccountId::from_xdr_base64(source_account_cstr.to_str()?)?;
    let src = Rc::new(ledger_storage::LedgerStorage {
        golang_handle: handle,
    });
    let storage = Storage::with_recording_footprint(src);
    let budget = Budget::default();
    let host = Host::with_storage_and_budget(storage, budget);

    host.switch_to_recording_auth();
    host.set_diagnostic_level(DiagnosticLevel::Debug);
    host.set_source_account(source_account);
    host.set_ledger_info(ledger_info.into());

    // Run the preflight.
    let result = host.invoke_function(invoke_hf_op.host_function.clone())?;
    let auths = host.get_recorded_auth_payloads()?;

    // Recover, convert and return the storage footprint and other values to C.
    let (storage, budget, events, _expiration_ledger_bumps) = host.try_finish().unwrap();

    let diagnostic_events = host_events_to_diagnostic_events(&events);
    let (transaction_data, min_fee) = fees::compute_host_function_transaction_data_and_min_fee(
        &InvokeHostFunctionOp {
            host_function: invoke_hf_op.host_function,
            auth: Default::default(),
        },
        &ledger_storage::LedgerStorage {
            golang_handle: handle,
        },
        &storage,
        &budget,
        &diagnostic_events,
    )?;
    let transaction_data_cstr = CString::new(transaction_data.to_xdr_base64()?)?;
    Ok(CPreflightResult {
        error: null_mut(),
        auth: recorded_auth_payloads_to_c(auths)?,
        result: CString::new(result.to_xdr_base64()?)?.into_raw(),
        transaction_data: transaction_data_cstr.into_raw(),
        min_fee,
        events: diagnostic_events_to_c(diagnostic_events)?,
        cpu_instructions: budget.get_cpu_insns_consumed(),
        memory_bytes: budget.get_mem_bytes_consumed(),
    })
}

#[no_mangle]
pub extern "C" fn preflight_footprint_expiration_op(
    handle: libc::uintptr_t, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHasconst
    op_body: *const libc::c_char, // OperationBody XDR in base64
    footprint: *const libc::c_char, // LedgerFootprint XDR in base64
) -> *mut CPreflightResult {
    catch_preflight_panic(Box::new(move || {
        preflight_footprint_expiration_op_or_maybe_panic(handle, op_body, footprint)
    }))
}

fn preflight_footprint_expiration_op_or_maybe_panic(
    handle: libc::uintptr_t,
    op_body: *const libc::c_char,
    footprint: *const libc::c_char,
) -> Result<CPreflightResult, Box<dyn error::Error>> {
    let op_body_cstr = unsafe { CStr::from_ptr(op_body) };
    let op_body = OperationBody::from_xdr_base64(op_body_cstr.to_str()?)?;
    let footprint_cstr = unsafe { CStr::from_ptr(footprint) };
    let ledger_footprint = LedgerFootprint::from_xdr_base64(footprint_cstr.to_str()?)?;
    let snapshot_source = &ledger_storage::LedgerStorage {
        golang_handle: handle,
    };
    match op_body {
        OperationBody::BumpFootprintExpiration(op) => preflight_bump_footprint_expiration(
            ledger_footprint,
            op.ledgers_to_expire,
            snapshot_source,
        ),
        OperationBody::RestoreFootprint(_) => {
            preflight_restore_footprint(ledger_footprint, snapshot_source)
        }
        op => Err(format!(
            "preflight_footprint_expiration_op(): unsupported operation type {}",
            op.name()
        )
        .into()),
    }
}

fn preflight_bump_footprint_expiration(
    footprint: LedgerFootprint,
    ledgers_to_expire: u32,
    snapshot_source: &LedgerStorage,
) -> Result<CPreflightResult, Box<dyn Error>> {
    let (transaction_data, min_fee) =
        fees::compute_bump_footprint_exp_transaction_data_and_min_fee(
            footprint,
            ledgers_to_expire,
            snapshot_source,
        )?;
    let transaction_data_cstr = CString::new(transaction_data.to_xdr_base64()?)?;
    Ok(CPreflightResult {
        error: null_mut(),
        auth: null_mut(),
        result: null_mut(),
        transaction_data: transaction_data_cstr.into_raw(),
        min_fee,
        events: null_mut(),
        cpu_instructions: 0,
        memory_bytes: 0,
    })
}

fn preflight_restore_footprint(
    footprint: LedgerFootprint,
    snapshot_source: &LedgerStorage,
) -> Result<CPreflightResult, Box<dyn Error>> {
    let (transaction_data, min_fee) =
        fees::compute_restore_footprint_transaction_data_and_min_fee(footprint, snapshot_source)?;
    let transaction_data_cstr = CString::new(transaction_data.to_xdr_base64()?)?;
    Ok(CPreflightResult {
        error: null_mut(),
        auth: null_mut(),
        result: null_mut(),
        transaction_data: transaction_data_cstr.into_raw(),
        min_fee,
        events: null_mut(),
        cpu_instructions: 0,
        memory_bytes: 0,
    })
}

fn catch_preflight_panic(
    op: Box<dyn Fn() -> Result<CPreflightResult, Box<dyn error::Error>>>,
) -> *mut CPreflightResult {
    // catch panics before they reach foreign callers (which otherwise would result in
    // undefined behavior)
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| op()));
    match res {
        Err(panic) => match panic.downcast::<String>() {
            Ok(panic_msg) => preflight_error(format!("panic during preflight() call: {panic_msg}")),
            Err(_) => preflight_error("panic during preflight() call: unknown cause".to_string()),
        },
        // transfer ownership to caller
        // caller needs to invoke free_preflight_result(result) when done
        Ok(r) => match r {
            Ok(r2) => Box::into_raw(Box::new(r2)),
            Err(e) => preflight_error(format!("{e}")),
        },
    }
}

fn recorded_auth_payloads_to_c(
    payloads: Vec<RecordedAuthPayload>,
) -> Result<*mut *mut libc::c_char, Box<dyn error::Error>> {
    let xdr_base64_vec: Vec<String> = payloads
        .iter()
        .map(|p| recorded_auth_payload_to_xdr(p).to_xdr_base64())
        .collect::<Result<Vec<_>, _>>()?;
    string_vec_to_c_null_terminated_char_array(xdr_base64_vec)
}

fn recorded_auth_payload_to_xdr(payload: &RecordedAuthPayload) -> SorobanAuthorizationEntry {
    match (payload.address.clone(), payload.nonce) {
        (Some(address), Some(nonce)) => SorobanAuthorizationEntry {
            credentials: SorobanCredentials::Address(SorobanAddressCredentials {
                address,
                nonce,
                // signature_args is left empty. This is where the client will put their signatures when
                // submitting the transaction.
                signature_expiration_ledger: 0,
                signature_args: ScVec::default(),
            }),
            root_invocation: payload.invocation.clone(),
        },
        _ => SorobanAuthorizationEntry {
            credentials: SorobanCredentials::SourceAccount,
            root_invocation: payload.invocation.clone(),
        },
        // TODO: there is a bug in the host library which prevents us from
        //       doing this check. It should be fixed in preview 11.
        // the address and the nonce can't be present independently
        // (a,n) =>
        //    panic!("recorded_auth_payload_to_xdr: address and nonce present independently (address: {:?}, nonce: {:?})", a, n),
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

fn diagnostic_events_to_c(
    events: Vec<DiagnosticEvent>,
) -> Result<*mut *mut libc::c_char, Box<dyn error::Error>> {
    let xdr_base64_vec: Vec<String> = events
        .iter()
        .map(DiagnosticEvent::to_xdr_base64)
        .collect::<Result<Vec<_>, _>>()?;
    string_vec_to_c_null_terminated_char_array(xdr_base64_vec)
}

fn string_vec_to_c_null_terminated_char_array(
    v: Vec<String>,
) -> Result<*mut *mut libc::c_char, Box<dyn error::Error>> {
    let mut out_vec: Vec<*mut libc::c_char> = Vec::new();
    for s in &v {
        let c_str = CString::new(s.clone())?.into_raw();
        out_vec.push(c_str);
    }

    // Add the ending NULL
    out_vec.push(null_mut());

    Ok(vec_to_c_array(out_vec))
}

fn vec_to_c_array<T>(mut v: Vec<T>) -> *mut T {
    // Make sure length and capacity are the same
    // (this allows using the length as the capacity when deallocating the vector)
    v.shrink_to_fit();
    assert_eq!(v.len(), v.capacity());

    // Get the pointer to our vector, we will deallocate it in free_c_null_terminated_char_array()
    // TODO: replace by `out_vec.into_raw_parts()` once the API stabilizes
    let ptr = v.as_mut_ptr();
    mem::forget(v);

    ptr
}

/// .
///
/// # Safety
///
/// .
#[no_mangle]
pub unsafe extern "C" fn free_preflight_result(result: *mut CPreflightResult) {
    if result.is_null() {
        return;
    }
    unsafe {
        if !(*result).error.is_null() {
            _ = CString::from_raw((*result).error);
        }

        if !(*result).auth.is_null() {
            free_c_null_terminated_char_array((*result).auth);
        }

        if !(*result).result.is_null() {
            _ = CString::from_raw((*result).result);
        }

        if !(*result).transaction_data.is_null() {
            _ = CString::from_raw((*result).transaction_data);
        }
        if !(*result).events.is_null() {
            free_c_null_terminated_char_array((*result).events);
        }
        _ = Box::from_raw(result);
    }
}

fn free_c_null_terminated_char_array(array: *mut *mut libc::c_char) {
    unsafe {
        // Iterate until we find a null value
        let mut i: usize = 0;
        loop {
            let c_char_ptr = *array.add(i);
            if c_char_ptr.is_null() {
                break;
            }
            // deallocate each string
            _ = CString::from_raw(c_char_ptr);
            i += 1;
        }
        // deallocate the containing vector
        _ = Vec::from_raw_parts(array, i + 1, i + 1);
    }
}
