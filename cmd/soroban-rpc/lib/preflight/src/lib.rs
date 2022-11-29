extern crate libc;
extern crate soroban_env_host;

use std::convert::TryInto;
use soroban_env_host::budget::Budget;
use soroban_env_host::storage::{self, AccessType, SnapshotSource, Storage};
use soroban_env_host::xdr::{self, AccountId, HostFunction, LedgerEntry, LedgerKey, ReadXdr, ScHostStorageErrorCode, ScVec, WriteXdr};
use soroban_env_host::{Host, HostError, LedgerInfo};
use std::ffi::{CStr, CString};
use std::panic;
use std::ptr::null_mut;
use std::rc::Rc;
use xdr::LedgerFootprint;

// TODO: we may want to pass callbacks instead of using global functions
extern "C" {
    // LedgerKey XDR in base64 string to LedgerEntry XDR in base64 string
    fn SnapshotSourceGet(ledger_key: *const libc::c_char) -> *const libc::c_char;
    // LedgerKey XDR in base64 string to bool
    fn SnapshotSourceHas(ledger_key: *const libc::c_char) -> libc::c_int;
    // Free Strings returned from Go functions
    fn FreeGoCString(str: *const libc::c_char);
}

struct CSnapshotSource;

impl SnapshotSource for CSnapshotSource {
    fn get(&self, key: &LedgerKey) -> Result<LedgerEntry, HostError> {
        let key_xdr = key.to_xdr_base64().unwrap();
        let key_cstr = CString::new(key_xdr).unwrap();
        let res = unsafe { SnapshotSourceGet(key_cstr.as_ptr()) };
        if res.is_null() {
            return Err(HostError::from(
                ScHostStorageErrorCode::AccessToUnknownEntry,
            ));
        }
        let res_cstr = unsafe { CStr::from_ptr(res) };
        let res_str = res_cstr.to_str().unwrap();
        // TODO: use a proper error
        let entry =
            LedgerEntry::from_xdr_base64(res_str).map_err(|_| ScHostStorageErrorCode::UnknownError)?;
        unsafe { FreeGoCString(res)};
        Ok(entry)
    }

    fn has(&self, key: &LedgerKey) -> Result<bool, HostError> {

        let key_xdr = key.to_xdr_base64().unwrap();
        let key_cstr = CString::new(key_xdr).unwrap();
        let res = unsafe { SnapshotSourceHas(key_cstr.as_ptr()) };
        Ok(match res {
            0 => false,
            _ => true,
        })
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
            network_passphrase: network_passphrase_cstr.to_str().unwrap().as_bytes().to_vec(),
            base_reserve: c.base_reserve,
        }
    }
}

fn storage_footprint_to_ledger_footprint(
    foot: &storage::Footprint,
) -> Result<LedgerFootprint, xdr::Error> {
    let mut read_only: Vec<LedgerKey> = Vec::new();
    let mut read_write: Vec<LedgerKey> = Vec::new();
    for (k, v) in foot.0.iter() {
        match v {
            AccessType::ReadOnly => read_only.push(*k.clone()),
            AccessType::ReadWrite => read_write.push(*k.clone()),
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
    pub cpu_instructions: u64,
    pub memory_bytes: u64,
}

fn preflight_error(str: String) -> *mut CPreflightResult {
    let c_str = CString::new(str).unwrap();
    // transfer ownership to caller
    // caller needs to invoke free_preflight_result(result) when done
    Box::into_raw(Box::new(CPreflightResult{
        error: c_str.into_raw(),
        result: null_mut(),
        footprint: null_mut(),
        cpu_instructions: 0,
        memory_bytes: 0,
    }))
}

#[no_mangle]
pub extern "C" fn preflight_host_function(
    hf: *const libc::c_char,   // HostFunction XDR in base64
    args: *const libc::c_char, // ScVec XDR in base64
    source_account: *const libc::c_char, // AccountId XDR in base64
    ledger_info: CLedgerInfo,
) -> *mut CPreflightResult
{
    // catch panics before they reach foreign callers (which otherwise would result in
    // undefined behavior)
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        preflight_host_function_or_maybe_panic(
            hf,
            args,
            source_account,
            ledger_info,
        )
    }));
    match res {
        Err(panic) =>
            match panic.downcast::<String>() {
                Ok(panic_msg) => {
                    preflight_error(format!("panic during preflight_host_function() call: {}", panic_msg))
                }
                Err(_) => {
                    preflight_error("panic during preflight_host_function() call: unknown cause".to_string())
                }
            },
        Ok(r) => r,
    }
}

fn preflight_host_function_or_maybe_panic(
    hf: *const libc::c_char,   // HostFunction XDR in base64
    args: *const libc::c_char, // ScVec XDR in base64
    source_account: *const libc::c_char, // AccountId XDR in base64
    ledger_info: CLedgerInfo,
) -> *mut CPreflightResult
{
    let hf_cstr = unsafe { CStr::from_ptr(hf) };
    let hf = match HostFunction::from_xdr_base64(hf_cstr.to_str().unwrap()) {
        Ok(hf) => hf,
        Err(err) => return preflight_error(format!("decoding host function: {}", err)),
    };
    let args_cstr = unsafe { CStr::from_ptr(args) };
    let args = match ScVec::from_xdr_base64(args_cstr.to_str().unwrap()) {
        Ok(args) => args,
        Err(err) => return preflight_error(format!("decoding args: {}", err)),
    };
    let source_account_cstr = unsafe { CStr::from_ptr(source_account) };
    let source_account = match AccountId::from_xdr_base64(source_account_cstr.to_str().unwrap()){
        Ok(account_id) => account_id,
        Err(err) => return preflight_error(format!("decoding account_id: {}", err)),
    };
    let src = Rc::new(CSnapshotSource);
    let storage = Storage::with_recording_footprint(src);
    let budget = Budget::default();
    let host = Host::with_storage_and_budget(storage, budget);

    host.set_source_account(source_account);
    host.set_ledger_info(ledger_info.into());

    // Run the preflight.
    let res = host.invoke_function(hf, args);

    // Recover, convert and return the storage footprint and other values to C.
    let (storage, budget, _) = match host.try_finish() {
        Ok(v) => v,
        Err(err) => {
            return preflight_error(format!("{:?}", err));
        }
    };

    let result = match res {
        Ok(val) => val,
        Err(err) => return preflight_error(err.to_string()),
    };

    let fp = match storage_footprint_to_ledger_footprint(&storage.footprint) {
        Ok(fp) => fp,
        Err(err) => {
            return preflight_error(err.to_string());
        }
    };
    let fp_cstr =  CString::new(fp.to_xdr_base64().unwrap()).unwrap();
    let result_cstr = CString::new(result.to_xdr_base64().unwrap()).unwrap();
    // transfer ownership to caller
    // caller needs to invoke free_preflight_result(result) when done
    Box::into_raw(Box::new(CPreflightResult{
        error: null_mut(),
        result: result_cstr.into_raw(),
        footprint: fp_cstr.into_raw(),
        cpu_instructions: budget.get_cpu_insns_count(),
        memory_bytes: budget.get_mem_bytes_count(),
    }))
}

#[no_mangle]
pub extern "C" fn free_preflight_result(result: *mut CPreflightResult) {
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
        let _ = Box::from_raw(result);
    }
}
