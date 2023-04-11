extern crate libc;
extern crate sha2;
extern crate soroban_env_host;

use sha2::{Digest, Sha256};
use soroban_env_host::auth::RecordedAuthPayload;
use soroban_env_host::budget::Budget;
use soroban_env_host::events::{Event, Events};
use soroban_env_host::storage::{self, AccessType, SnapshotSource, Storage};
use soroban_env_host::xdr::{
    self, AccountId, AddressWithNonce, ContractAuth, DiagnosticEvent, HostFunction, LedgerEntry,
    LedgerKey, ReadXdr, ScHostStorageErrorCode, ScStatus,
    ScUnknownErrorCode::{General, Xdr},
    WriteXdr,
};
use soroban_env_host::{Host, HostError, LedgerInfo};
use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::panic;
use std::ptr::null_mut;
use std::rc::Rc;
use std::{error, mem};
use xdr::LedgerFootprint;

extern "C" {
    // LedgerKey XDR in base64 string to LedgerEntry XDR in base64 string
    fn SnapshotSourceGet(
        handle: libc::uintptr_t,
        ledger_key: *const libc::c_char,
    ) -> *const libc::c_char;
    // LedgerKey XDR in base64 string to bool
    fn SnapshotSourceHas(handle: libc::uintptr_t, ledger_key: *const libc::c_char) -> libc::c_int;
    // Free Strings returned from Go functions
    fn FreeGoCString(str: *const libc::c_char);
}

struct CSnapshotSource {
    handle: libc::uintptr_t,
}

impl SnapshotSource for CSnapshotSource {
    fn get(&self, key: &Rc<LedgerKey>) -> Result<Rc<LedgerEntry>, HostError> {
        let key_xdr = key
            .to_xdr_base64()
            .map_err(|_| ScStatus::UnknownError(Xdr))?;
        let key_cstr = CString::new(key_xdr).map_err(|_| ScStatus::UnknownError(General))?;
        let res = unsafe { SnapshotSourceGet(self.handle, key_cstr.as_ptr()) };
        if res.is_null() {
            return Err(HostError::from(
                ScHostStorageErrorCode::AccessToUnknownEntry,
            ));
        }
        let res_cstr = unsafe { CStr::from_ptr(res) };
        let res_str = res_cstr
            .to_str()
            .map_err(|_| ScStatus::UnknownError(General))?;
        let entry =
            LedgerEntry::from_xdr_base64(res_str).map_err(|_| ScStatus::UnknownError(Xdr))?;
        unsafe { FreeGoCString(res) };
        Ok(entry.into())
    }

    fn has(&self, key: &Rc<LedgerKey>) -> Result<bool, HostError> {
        let key_xdr = key
            .to_xdr_base64()
            .map_err(|_| ScStatus::UnknownError(Xdr))?;
        let key_cstr = CString::new(key_xdr).map_err(|_| ScStatus::UnknownError(Xdr))?;
        let res = unsafe { SnapshotSourceHas(self.handle, key_cstr.as_ptr()) };
        Ok(res != 0)
    }
}

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

fn storage_footprint_to_ledger_footprint(
    foot: &storage::Footprint,
) -> Result<LedgerFootprint, xdr::Error> {
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

#[repr(C)]
pub struct CPreflightResult {
    pub error: *mut libc::c_char, // Error string in case of error, otherwise null
    pub result: *mut libc::c_char, // SCVal XDR in base64
    pub footprint: *mut libc::c_char, // LedgerFootprint XDR in base64
    pub auth: *mut *mut libc::c_char, // NULL terminated array of XDR ContractAuths in base64
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
        result: null_mut(),
        footprint: null_mut(),
        auth: null_mut(),
        events: null_mut(),
        cpu_instructions: 0,
        memory_bytes: 0,
    }))
}

#[no_mangle]
pub extern "C" fn preflight_host_function(
    handle: libc::uintptr_t, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHasconst
    hf: *const libc::c_char, // HostFunction XDR in base64
    source_account: *const libc::c_char, // AccountId XDR in base64
    ledger_info: CLedgerInfo,
) -> *mut CPreflightResult {
    // catch panics before they reach foreign callers (which otherwise would result in
    // undefined behavior)
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        preflight_host_function_or_maybe_panic(handle, hf, source_account, ledger_info)
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

fn preflight_host_function_or_maybe_panic(
    handle: libc::uintptr_t, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHas
    hf: *const libc::c_char, // HostFunction XDR in base64
    source_account: *const libc::c_char, // AccountId XDR in base64
    ledger_info: CLedgerInfo,
) -> Result<CPreflightResult, Box<dyn error::Error>> {
    let hf_cstr = unsafe { CStr::from_ptr(hf) };
    let hf = HostFunction::from_xdr_base64(hf_cstr.to_str()?)?;
    let source_account_cstr = unsafe { CStr::from_ptr(source_account) };
    let source_account = AccountId::from_xdr_base64(source_account_cstr.to_str()?)?;
    let src = Rc::new(CSnapshotSource { handle });
    let storage = Storage::with_recording_footprint(src);
    let budget = Budget::default();
    let host = Host::with_storage_and_budget(storage, budget);

    host.switch_to_recording_auth();
    host.set_source_account(source_account);
    host.set_ledger_info(ledger_info.into());

    // Run the preflight.
    let result = host.invoke_function(hf)?;
    let auth_payloads = host.get_recorded_auth_payloads()?;

    // Recover, convert and return the storage footprint and other values to C.
    let (storage, budget, events) = host.try_finish().unwrap();

    let fp = storage_footprint_to_ledger_footprint(&storage.footprint)?;
    let fp_cstr = CString::new(fp.to_xdr_base64()?)?;
    let result_cstr = CString::new(result.to_xdr_base64()?)?;
    Ok(CPreflightResult {
        error: null_mut(),
        result: result_cstr.into_raw(),
        footprint: fp_cstr.into_raw(),
        auth: recorded_auth_payloads_to_c(auth_payloads)?,
        events: host_events_to_c(events)?,
        cpu_instructions: budget.get_cpu_insns_count(),
        memory_bytes: budget.get_mem_bytes_count(),
    })
}

fn recorded_auth_payloads_to_c(
    payloads: Vec<RecordedAuthPayload>,
) -> Result<*mut *mut libc::c_char, Box<dyn error::Error>> {
    let xdr_base64_vec: Vec<String> = payloads
        .iter()
        .map(|p| recorded_auth_payload_to_xdr(p).to_xdr_base64())
        .collect::<Result<Vec<_>, _>>()?;
    string_vec_to_c_to_null_terminated_char_array(xdr_base64_vec)
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

fn host_events_to_c(events: Events) -> Result<*mut *mut libc::c_char, Box<dyn error::Error>> {
    let mut xdr_base64_vec: Vec<String> = Vec::new();
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
            xdr_base64_vec.push(diagnostic_event.to_xdr_base64()?);
        }
    }
    string_vec_to_c_to_null_terminated_char_array(xdr_base64_vec)
}

fn string_vec_to_c_to_null_terminated_char_array(
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
        if !(*result).result.is_null() {
            let _ = CString::from_raw((*result).result);
        }
        if !(*result).footprint.is_null() {
            let _ = CString::from_raw((*result).footprint);
        }
        if !(*result).auth.is_null() {
            free_c_null_terminated_char_array((*result).auth);
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
            // deallocate each base64 string
            let _ = CString::from_raw(c_char_ptr);
            i += 1;
        }
        // deallocate the containing vector
        let _ = Vec::from_raw_parts(array, i + 1, i + 1);
    }
}
