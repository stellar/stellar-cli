mod fees;
mod ledger_storage;

extern crate base64;
extern crate libc;
extern crate sha2;
extern crate soroban_env_host;

use sha2::{Digest, Sha256};
use soroban_env_host::auth::RecordedAuthPayload;
use soroban_env_host::budget::Budget;
use soroban_env_host::events::{Event, Events};
use soroban_env_host::storage::Storage;
use soroban_env_host::xdr::{
    AccountId, AddressWithNonce, ContractAuth, DiagnosticEvent, HostFunction, InvokeHostFunctionOp,
    ReadXdr, ScVal, WriteXdr,
};
use soroban_env_host::{Host, LedgerInfo};
use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::panic;
use std::ptr::null_mut;
use std::rc::Rc;
use std::{error, mem};

#[repr(C)]
pub struct CLedgerInfo {
    pub protocol_version: u32,
    pub sequence_number: u32,
    pub timestamp: u64,
    pub network_passphrase: *const libc::c_char,
    pub base_reserve: u32,
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
        }
    }
}

#[repr(C)]
pub struct CHostFunctionPreflight {
    pub auth: *mut *mut libc::c_char, // NULL terminated array of XDR ContractAuths in base64
    pub result: *mut libc::c_char,    // XDR SCVal in base64
}

#[repr(C)]
pub struct CPreflightResult {
    pub error: *mut libc::c_char, // Error string in case of error, otherwise null
    pub results: *mut CHostFunctionPreflight, // array of CHostFunctionPreflight
    pub results_size: libc::size_t,
    pub transaction_data: *mut libc::c_char, // SorobanTransactionData XDR in base64
    pub min_fee: i64,                        // Minimum recommended resource fee
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
        results: null_mut(),
        results_size: 0,
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
    // catch panics before they reach foreign callers (which otherwise would result in
    // undefined behavior)
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        preflight_invoke_hf_op_or_maybe_panic(handle, invoke_hf_op, source_account, ledger_info)
    }));
    match res {
        Err(panic) => match panic.downcast::<String>() {
            Ok(panic_msg) => preflight_error(format!(
                "panic during preflight_host_function() call: {panic_msg}"
            )),
            Err(_) => preflight_error(
                "panic during preflight_host_function() call: unknown cause".to_string(),
            ),
        },
        // transfer ownership to caller
        // caller needs to invoke free_preflight_result(result) when done
        Ok(r) => match r {
            Ok(r2) => Box::into_raw(Box::new(r2)),
            Err(e) => preflight_error(format!("{e}")),
        },
    }
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
    host.set_source_account(source_account);
    host.set_ledger_info(ledger_info.into());

    // Add auths to the functions, so that they are later taken into account for
    // the envelope size estimation
    let mut results_and_auths: Vec<(ScVal, Vec<RecordedAuthPayload>)> = Vec::new();

    // Run the preflight.
    let mut functions_with_auths: Vec<HostFunction> = invoke_hf_op.functions.clone().into_vec();
    for (i, f) in invoke_hf_op.functions.as_vec().iter().enumerate() {
        // Hack to obtain the auth of each function separately
        host.switch_to_recording_auth(); // resets auth from previous calls
        let results = host.invoke_functions(vec![f.clone()])?;
        let auths = host.get_recorded_auth_payloads()?;
        functions_with_auths[i].auth = auths
            .iter()
            .map(recorded_auth_payload_to_xdr)
            .collect::<Vec<_>>()
            .try_into()?;
        results_and_auths.push((results[0].clone(), auths));
    }

    // Recover, convert and return the storage footprint and other values to C.
    let (storage, budget, events) = host.try_finish().unwrap();

    let diagnostic_events = host_events_to_diagnostic_events(&events)?;
    let (transaction_data, min_fee) = fees::compute_transaction_data_and_min_fee(
        &InvokeHostFunctionOp {
            functions: functions_with_auths.try_into()?,
        },
        &ledger_storage::LedgerStorage {
            golang_handle: handle,
        },
        &storage,
        &budget,
        &diagnostic_events,
    )?;
    let transaction_data_cstr = CString::new(transaction_data.to_xdr_base64()?)?;
    let (results, results_size) = get_c_host_function_preflight_array(results_and_auths)?;
    Ok(CPreflightResult {
        error: null_mut(),
        results: results,
        results_size: results_size,
        transaction_data: transaction_data_cstr.into_raw(),
        min_fee: min_fee,
        events: diagnostic_events_to_c(diagnostic_events)?,
        cpu_instructions: budget.get_cpu_insns_count(),
        memory_bytes: budget.get_mem_bytes_count(),
    })
}

fn recorded_auth_payloads_to_c(
    payloads: &Vec<RecordedAuthPayload>,
) -> Result<*mut *mut libc::c_char, Box<dyn error::Error>> {
    let xdr_base64_vec: Vec<String> = payloads
        .iter()
        .map(|p| recorded_auth_payload_to_xdr(p).to_xdr_base64())
        .collect::<Result<Vec<_>, _>>()?;
    string_vec_to_c_null_terminated_char_array(xdr_base64_vec)
}

fn recorded_auth_payload_to_xdr(payload: &RecordedAuthPayload) -> ContractAuth {
    let address_with_nonce = match (payload.address.clone(), payload.nonce) {
        (Some(address), Some(nonce)) => Some(AddressWithNonce { address, nonce }),
        (None, None) => None,
        // the address and the nonce can't be present independently
        (a,n) =>
            panic!("recorded_auth_payload_to_xdr: address and nonce present independently (address: {:?}, nonce: {:?})", a, n),
    };
    ContractAuth {
        address_with_nonce,
        root_invocation: payload.invocation.clone(),
        // signature_args is left empty. This is where the client will put their signatures when
        // submitting the transaction.
        signature_args: Default::default(),
    }
}

fn host_events_to_diagnostic_events(
    events: &Events,
) -> Result<Vec<DiagnosticEvent>, Box<dyn error::Error>> {
    let mut res: Vec<DiagnosticEvent> = Vec::new();
    for e in events.0.iter() {
        let maybe_contract_event = match &e.event {
            Event::Contract(e) => Some(e),
            Event::StructuredDebug(e) => Some(e),
            // Debug events can't be translated to diagnostic events
            Event::Debug(_) => None,
        };
        if let Some(contract_event) = maybe_contract_event {
            let diagnostic_event = DiagnosticEvent {
                in_successful_contract_call: !e.failed_call,
                event: contract_event.clone(),
            };
            res.push(diagnostic_event);
        }
    }
    Ok(res)
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

// TODO: can we factor out the common parts of get_c_null_terminated_host_function_preflight_array and
//       string_vec_to_c_null_terminated_char_array ?
fn get_c_host_function_preflight_array(
    results_and_auths: Vec<(ScVal, Vec<RecordedAuthPayload>)>,
) -> Result<(*mut CHostFunctionPreflight, libc::size_t), Box<dyn error::Error>> {
    let mut out_vec: Vec<CHostFunctionPreflight> = Vec::new();
    for (result, auths) in results_and_auths {
        let result_c_str = CString::new(result.to_xdr_base64()?)?.into_raw();
        out_vec.push(CHostFunctionPreflight {
            auth: recorded_auth_payloads_to_c(&auths)?,
            result: result_c_str,
        });
    }

    // Make sure length and capacity are the same
    // (this allows using the length as the capacity when deallocating the vector)
    out_vec.shrink_to_fit();
    assert_eq!(out_vec.len(), out_vec.capacity());

    // Get the pointer to our vector, we will deallocate it in free_c_null_terminated_char_array()
    // TODO: replace by `out_vec.into_raw_parts()` once the API stabilizes
    let ptr = out_vec.as_mut_ptr();
    let size = out_vec.len();
    mem::forget(out_vec);

    Ok((ptr, size))
}

fn string_vec_to_c_null_terminated_char_array(
    v: Vec<String>,
) -> Result<*mut *mut libc::c_char, Box<dyn error::Error>> {
    let mut out_vec: Vec<*mut libc::c_char> = Vec::new();
    for s in v.iter() {
        let c_str = CString::new(s.clone())?.into_raw();
        out_vec.push(c_str);
    }

    // Add the ending NULL
    out_vec.push(null_mut());

    // Make sure length and capacity are the same
    // (this allows using the length as the capacity when deallocating the vector)
    out_vec.shrink_to_fit();
    assert_eq!(out_vec.len(), out_vec.capacity());

    // Get the pointer to our vector, we will deallocate it in free_c_null_terminated_char_array()
    // TODO: replace by `out_vec.into_raw_parts()` once the API stabilizes
    let ptr = out_vec.as_mut_ptr();
    mem::forget(out_vec);

    Ok(ptr)
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
            let _ = CString::from_raw((*result).error);
        }
        if !(*result).results.is_null() {
            let results = Vec::from_raw_parts(
                (*result).results,
                (*result).results_size,
                (*result).results_size,
            );
            for result in results.iter() {
                free_c_null_terminated_char_array(result.auth);
                let _ = CString::from_raw(result.result);
            }
        }
        if !(*result).transaction_data.is_null() {
            let _ = CString::from_raw((*result).transaction_data);
        }
        if !(*result).events.is_null() {
            free_c_null_terminated_char_array((*result).events);
        }
        let _ = Box::from_raw(result);
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
            let _ = CString::from_raw(c_char_ptr);
            i += 1;
        }
        // deallocate the containing vector
        let _ = Vec::from_raw_parts(array, i + 1, i + 1);
    }
}
