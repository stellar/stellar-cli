mod fees;
mod ledger_storage;
mod preflight;
mod state_expiration;

extern crate anyhow;
extern crate base64;
extern crate libc;
extern crate sha2;
extern crate soroban_env_host;
use anyhow::{Context, Result};
use ledger_storage::LedgerStorage;
use preflight::PreflightResult;
use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{
    AccountId, InvokeHostFunctionOp, LedgerFootprint, OperationBody, ReadXdr, WriteXdr,
};
use soroban_env_host::LedgerInfo;
use std::convert::{TryFrom, TryInto};
use std::ffi::{CStr, CString};
use std::mem;
use std::panic;
use std::ptr::null_mut;

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
    pub autobump_ledgers: u32,
}

impl TryFrom<CLedgerInfo> for LedgerInfo {
    type Error = anyhow::Error;

    fn try_from(c: CLedgerInfo) -> Result<Self> {
        let network_passphrase = from_c_string(c.network_passphrase)?;
        Ok(Self {
            protocol_version: c.protocol_version,
            sequence_number: c.sequence_number,
            timestamp: c.timestamp,
            network_id: Sha256::digest(network_passphrase).into(),
            base_reserve: c.base_reserve,
            min_temp_entry_expiration: c.min_temp_entry_expiration,
            min_persistent_entry_expiration: c.min_persistent_entry_expiration,
            max_entry_expiration: c.max_entry_expiration,
            autobump_ledgers: c.autobump_ledgers,
        })
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
    pub pre_restore_transaction_data: *mut libc::c_char, // SorobanTransactionData XDR in base64 for a prerequired RestoreFootprint operation
    pub pre_restore_min_fee: i64, // Minimum recommended resource fee for a prerequired RestoreFootprint operation
}

impl TryFrom<PreflightResult> for CPreflightResult {
    type Error = anyhow::Error;

    fn try_from(p: PreflightResult) -> Result<Self> {
        let mut result = Self {
            error: null_mut(),
            auth: xdr_vec_to_base64_c_null_terminated_char_array(p.auth)?,
            result: match p.result {
                None => null_mut(),
                Some(v) => xdr_to_base64_c(v)?,
            },
            transaction_data: xdr_to_base64_c(p.transaction_data)?,
            min_fee: p.min_fee,
            events: xdr_vec_to_base64_c_null_terminated_char_array(p.events)?,
            cpu_instructions: p.cpu_instructions,
            memory_bytes: p.memory_bytes,
            pre_restore_transaction_data: null_mut(),
            pre_restore_min_fee: 0,
        };
        if let Some(p) = p.restore_preamble {
            result.pre_restore_min_fee = p.min_fee;
            result.pre_restore_transaction_data = xdr_to_base64_c(p.transaction_data)?;
        };
        Ok(result)
    }
}

#[no_mangle]
pub extern "C" fn preflight_invoke_hf_op(
    handle: libc::uintptr_t, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHas
    bucket_list_size: u64,   // Bucket list size for current ledger
    invoke_hf_op: *const libc::c_char, // InvokeHostFunctionOp XDR in base64
    source_account: *const libc::c_char, // AccountId XDR in base64
    ledger_info: CLedgerInfo,
) -> *mut CPreflightResult {
    println!("gets to top preflight_invoke_hf_op");
    catch_preflight_panic(Box::new(move || {
        preflight_invoke_hf_op_or_maybe_panic(
            handle,
            bucket_list_size,
            invoke_hf_op,
            source_account,
            ledger_info,
        )
    }))
}

fn preflight_invoke_hf_op_or_maybe_panic(
    handle: libc::uintptr_t,
    bucket_list_size: u64, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHas
    invoke_hf_op: *const libc::c_char, // InvokeHostFunctionOp XDR in base64
    source_account: *const libc::c_char, // AccountId XDR in base64
    ledger_info: CLedgerInfo,
) -> Result<CPreflightResult> {
    let invoke_hf_op = InvokeHostFunctionOp::from_xdr_base64(from_c_string(invoke_hf_op)?)?;
    let source_account = AccountId::from_xdr_base64(from_c_string(source_account)?)?;
    let ledger_storage = LedgerStorage::with_restore_tracking(handle, ledger_info.sequence_number)
        .context("cannot create LedgerStorage")?;
    let result = preflight::preflight_invoke_hf_op(
        ledger_storage,
        bucket_list_size,
        invoke_hf_op,
        source_account,
        ledger_info.try_into()?,
    )?;
    result.try_into()
}

#[no_mangle]
pub extern "C" fn preflight_footprint_expiration_op(
    handle: libc::uintptr_t, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHas
    bucket_list_size: u64,   // Bucket list size for current ledger
    op_body: *const libc::c_char, // OperationBody XDR in base64
    footprint: *const libc::c_char, // LedgerFootprint XDR in base64
    current_ledger_seq: u32,
) -> *mut CPreflightResult {
    catch_preflight_panic(Box::new(move || {
        preflight_footprint_expiration_op_or_maybe_panic(
            handle,
            bucket_list_size,
            op_body,
            footprint,
            current_ledger_seq,
        )
    }))
}

fn preflight_footprint_expiration_op_or_maybe_panic(
    handle: libc::uintptr_t,
    bucket_list_size: u64,
    op_body: *const libc::c_char,
    footprint: *const libc::c_char,
    current_ledger_seq: u32,
) -> Result<CPreflightResult> {
    let op_body = OperationBody::from_xdr_base64(from_c_string(op_body)?)?;
    let footprint = LedgerFootprint::from_xdr_base64(from_c_string(footprint)?)?;
    let ledger_storage = &LedgerStorage::new(handle);
    let result = preflight::preflight_footprint_expiration_op(
        ledger_storage,
        bucket_list_size,
        op_body,
        footprint,
        current_ledger_seq,
    )?;
    result.try_into()
}

fn preflight_error(str: String) -> CPreflightResult {
    let c_str = CString::new(str).unwrap();
    CPreflightResult {
        error: c_str.into_raw(),
        auth: null_mut(),
        result: null_mut(),
        transaction_data: null_mut(),
        min_fee: 0,
        events: null_mut(),
        cpu_instructions: 0,
        memory_bytes: 0,
        pre_restore_transaction_data: null_mut(),
        pre_restore_min_fee: 0,
    }
}

fn catch_preflight_panic(op: Box<dyn Fn() -> Result<CPreflightResult>>) -> *mut CPreflightResult {
    // catch panics before they reach foreign callers (which otherwise would result in
    // undefined behavior)
    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| op()));
    let c_preflight_result = match res {
        Err(panic) => match panic.downcast::<String>() {
            Ok(panic_msg) => preflight_error(format!("panic during preflight() call: {panic_msg}")),
            Err(_) => preflight_error("panic during preflight() call: unknown cause".to_string()),
        },
        Ok(r) => match r {
            Ok(r2) => r2,
            Err(e) => preflight_error(format!("{e}")),
        },
    };
    // transfer ownership to caller
    // caller needs to invoke free_preflight_result(result) when done
    Box::into_raw(Box::new(c_preflight_result))
}

fn xdr_to_base64_c(v: impl WriteXdr) -> Result<*mut libc::c_char> {
    string_to_c(v.to_xdr_base64()?)
}

fn string_to_c(str: String) -> Result<*mut libc::c_char> {
    Ok(CString::new(str)?.into_raw())
}

fn xdr_vec_to_base64_c_null_terminated_char_array(
    payloads: Vec<impl WriteXdr>,
) -> Result<*mut *mut libc::c_char> {
    let xdr_base64_vec: Vec<String> = payloads
        .iter()
        .map(WriteXdr::to_xdr_base64)
        .collect::<Result<Vec<_>, _>>()?;
    string_vec_to_c_null_terminated_char_array(xdr_base64_vec)
}

fn string_vec_to_c_null_terminated_char_array(v: Vec<String>) -> Result<*mut *mut libc::c_char> {
    let mut out_vec: Vec<*mut libc::c_char> = Vec::new();
    for s in &v {
        let c_str = string_to_c(s.clone())?;
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
    let boxed = Box::from_raw(result);
    free_c_string(boxed.error);
    free_c_null_terminated_char_array(boxed.auth);
    free_c_string(boxed.result);
    free_c_string(boxed.transaction_data);
    free_c_null_terminated_char_array(boxed.events);
    free_c_string(boxed.pre_restore_transaction_data);
}

fn free_c_string(str: *mut libc::c_char) {
    if str.is_null() {
        return;
    }
    unsafe {
        _ = CString::from_raw(str);
    }
}

fn free_c_null_terminated_char_array(array: *mut *mut libc::c_char) {
    if array.is_null() {
        return;
    }
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
fn from_c_string(str: *const libc::c_char) -> Result<String> {
    let c_str = unsafe { CStr::from_ptr(str) };
    Ok(c_str.to_str()?.to_string())
}
