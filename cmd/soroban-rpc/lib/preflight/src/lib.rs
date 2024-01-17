extern crate base64;
extern crate libc;
extern crate sha2;
extern crate soroban_env_host;
extern crate soroban_simulation;

use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{
    AccountId, Hash, InvokeHostFunctionOp, LedgerEntry, LedgerEntryData, LedgerFootprint,
    LedgerKey, LedgerKeyTtl, Limits, OperationBody, ReadXdr, TtlEntry, WriteXdr,
};
use soroban_env_host::LedgerInfo;
use soroban_simulation::{ledger_storage, ResourceConfig};
use soroban_simulation::{
    simulate_footprint_ttl_op, simulate_invoke_hf_op, LedgerStorage, SimulationResult,
};
use std::error::Error;
use std::ffi::{CStr, CString};
use std::panic;
use std::ptr::null_mut;
use std::{mem, slice};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CLedgerInfo {
    pub protocol_version: u32,
    pub sequence_number: u32,
    pub timestamp: u64,
    pub network_passphrase: *const libc::c_char,
    pub base_reserve: u32,
    pub min_temp_entry_ttl: u32,
    pub min_persistent_entry_ttl: u32,
    pub max_entry_ttl: u32,
}

impl From<CLedgerInfo> for LedgerInfo {
    fn from(c: CLedgerInfo) -> Self {
        let network_passphrase = from_c_string(c.network_passphrase);
        Self {
            protocol_version: c.protocol_version,
            sequence_number: c.sequence_number,
            timestamp: c.timestamp,
            network_id: Sha256::digest(network_passphrase).into(),
            base_reserve: c.base_reserve,
            min_temp_entry_ttl: c.min_temp_entry_ttl,
            min_persistent_entry_ttl: c.min_persistent_entry_ttl,
            max_entry_ttl: c.max_entry_ttl,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CXDR {
    pub xdr: *mut libc::c_uchar,
    pub len: libc::size_t,
}

// It would be nicer to derive Default, but we can't. It errors with:
// The trait bound `*mut u8: std::default::Default` is not satisfied
fn get_default_c_xdr() -> CXDR {
    CXDR {
        xdr: null_mut(),
        len: 0,
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CXDRVector {
    pub array: *mut CXDR,
    pub len: libc::size_t,
}

fn get_default_c_xdr_vector() -> CXDRVector {
    CXDRVector {
        array: null_mut(),
        len: 0,
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CResourceConfig {
    pub instruction_leeway: u64,
}

impl From<CResourceConfig> for ResourceConfig {
    fn from(r: CResourceConfig) -> Self {
        return ResourceConfig {
            instruction_leeway: r.instruction_leeway,
        };
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CPreflightResult {
    // Error string in case of error, otherwise null
    pub error: *mut libc::c_char,
    // Error string in case of error, otherwise null
    pub auth: CXDRVector,
    // XDR SCVal
    pub result: CXDR,
    // SorobanTransactionData XDR
    pub transaction_data: CXDR,
    // Minimum recommended resource fee
    pub min_fee: i64,
    // array of XDR ContractEvents
    pub events: CXDRVector,
    pub cpu_instructions: u64,
    pub memory_bytes: u64,
    // SorobanTransactionData XDR for a prerequired RestoreFootprint operation
    pub pre_restore_transaction_data: CXDR,
    // Minimum recommended resource fee for a prerequired RestoreFootprint operation
    pub pre_restore_min_fee: i64,
}

impl From<SimulationResult> for CPreflightResult {
    fn from(s: SimulationResult) -> Self {
        let mut result = Self {
            error: string_to_c(s.error),
            auth: xdr_vec_to_c(s.auth),
            result: option_xdr_to_c(s.result),
            transaction_data: option_xdr_to_c(s.transaction_data),
            min_fee: s.min_fee,
            events: xdr_vec_to_c(s.events),
            cpu_instructions: s.cpu_instructions,
            memory_bytes: s.memory_bytes,
            pre_restore_transaction_data: get_default_c_xdr(),
            pre_restore_min_fee: 0,
        };
        if let Some(p) = s.restore_preamble {
            result.pre_restore_min_fee = p.min_fee;
            result.pre_restore_transaction_data = xdr_to_c(p.transaction_data);
        };
        result
    }
}

#[no_mangle]
pub extern "C" fn preflight_invoke_hf_op(
    handle: libc::uintptr_t, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHas
    bucket_list_size: u64,   // Bucket list size for current ledger
    invoke_hf_op: CXDR,      // InvokeHostFunctionOp XDR in base64
    source_account: CXDR,    // AccountId XDR in base64
    ledger_info: CLedgerInfo,
    resource_config: CResourceConfig,
    enable_debug: bool,
) -> *mut CPreflightResult {
    catch_preflight_panic(Box::new(move || {
        preflight_invoke_hf_op_or_maybe_panic(
            handle,
            bucket_list_size,
            invoke_hf_op,
            source_account,
            ledger_info,
            resource_config,
            enable_debug,
        )
    }))
}

fn preflight_invoke_hf_op_or_maybe_panic(
    handle: libc::uintptr_t,
    bucket_list_size: u64, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHas
    invoke_hf_op: CXDR,    // InvokeHostFunctionOp XDR in base64
    source_account: CXDR,  // AccountId XDR in base64
    ledger_info: CLedgerInfo,
    resource_config: CResourceConfig,
    enable_debug: bool,
) -> Result<CPreflightResult, Box<dyn Error>> {
    let invoke_hf_op =
        InvokeHostFunctionOp::from_xdr(from_c_xdr(invoke_hf_op), Limits::none()).unwrap();
    let source_account = AccountId::from_xdr(from_c_xdr(source_account), Limits::none()).unwrap();
    let go_storage = GoLedgerStorage {
        golang_handle: handle,
        current_ledger_sequence: ledger_info.sequence_number,
    };
    let ledger_storage =
        LedgerStorage::with_restore_tracking(Box::new(go_storage), ledger_info.sequence_number)?;
    let result = simulate_invoke_hf_op(
        ledger_storage,
        bucket_list_size,
        invoke_hf_op,
        source_account,
        LedgerInfo::from(ledger_info),
        resource_config.into(),
        enable_debug,
    );
    match result {
        Ok(r) => Ok(r.into()),
        Err(e) => Err(e),
    }
}

#[no_mangle]
pub extern "C" fn preflight_footprint_ttl_op(
    handle: libc::uintptr_t, // Go Handle to forward to SnapshotSourceGet and SnapshotSourceHas
    bucket_list_size: u64,   // Bucket list size for current ledger
    op_body: CXDR,           // OperationBody XDR
    footprint: CXDR,         // LedgerFootprint XDR
    current_ledger_seq: u32,
) -> *mut CPreflightResult {
    catch_preflight_panic(Box::new(move || {
        preflight_footprint_ttl_op_or_maybe_panic(
            handle,
            bucket_list_size,
            op_body,
            footprint,
            current_ledger_seq,
        )
    }))
}

fn preflight_footprint_ttl_op_or_maybe_panic(
    handle: libc::uintptr_t,
    bucket_list_size: u64,
    op_body: CXDR,
    footprint: CXDR,
    current_ledger_seq: u32,
) -> Result<CPreflightResult, Box<dyn Error>> {
    let op_body = OperationBody::from_xdr(from_c_xdr(op_body), Limits::none()).unwrap();
    let footprint = LedgerFootprint::from_xdr(from_c_xdr(footprint), Limits::none()).unwrap();
    let go_storage = GoLedgerStorage {
        golang_handle: handle,
        current_ledger_sequence: current_ledger_seq,
    };
    let ledger_storage = &LedgerStorage::new(Box::new(go_storage), current_ledger_seq);
    let result = simulate_footprint_ttl_op(
        ledger_storage,
        bucket_list_size,
        op_body,
        footprint,
        current_ledger_seq,
    );
    match result {
        Ok(r) => Ok(r.into()),
        Err(e) => Err(e),
    }
}

fn preflight_error(str: String) -> CPreflightResult {
    let c_str = CString::new(str).unwrap();
    CPreflightResult {
        error: c_str.into_raw(),
        auth: get_default_c_xdr_vector(),
        result: get_default_c_xdr(),
        transaction_data: get_default_c_xdr(),
        min_fee: 0,
        events: get_default_c_xdr_vector(),
        cpu_instructions: 0,
        memory_bytes: 0,
        pre_restore_transaction_data: get_default_c_xdr(),
        pre_restore_min_fee: 0,
    }
}

fn catch_preflight_panic(
    op: Box<dyn Fn() -> Result<CPreflightResult, Box<dyn Error>>>,
) -> *mut CPreflightResult {
    // catch panics before they reach foreign callers (which otherwise would result in
    // undefined behavior)
    let res: std::thread::Result<Result<CPreflightResult, Box<dyn Error>>> =
        panic::catch_unwind(panic::AssertUnwindSafe(op));
    let c_preflight_result = match res {
        Err(panic) => match panic.downcast::<String>() {
            Ok(panic_msg) => preflight_error(format!("panic during preflight() call: {panic_msg}")),
            Err(_) => preflight_error("panic during preflight() call: unknown cause".to_string()),
        },
        // See https://docs.rs/anyhow/latest/anyhow/struct.Error.html#display-representations
        Ok(r) => r.unwrap_or_else(|e| preflight_error(format!("{e:?}"))),
    };
    // transfer ownership to caller
    // caller needs to invoke free_preflight_result(result) when done
    Box::into_raw(Box::new(c_preflight_result))
}

fn xdr_to_c(v: impl WriteXdr) -> CXDR {
    let (xdr, len) = vec_to_c_array(v.to_xdr(Limits::none()).unwrap());
    CXDR { xdr, len }
}

fn option_xdr_to_c(v: Option<impl WriteXdr>) -> CXDR {
    v.map_or(
        CXDR {
            xdr: null_mut(),
            len: 0,
        },
        xdr_to_c,
    )
}

fn xdr_vec_to_c(v: Vec<impl WriteXdr>) -> CXDRVector {
    let c_v = v.into_iter().map(xdr_to_c).collect();
    let (array, len) = vec_to_c_array(c_v);
    CXDRVector { array, len }
}

fn string_to_c(str: String) -> *mut libc::c_char {
    CString::new(str).unwrap().into_raw()
}

fn vec_to_c_array<T>(mut v: Vec<T>) -> (*mut T, libc::size_t) {
    // Make sure length and capacity are the same
    // (this allows using the length as the capacity when deallocating the vector)
    v.shrink_to_fit();
    let len = v.len();
    assert_eq!(len, v.capacity());

    // Get the pointer to our vector, we will deallocate it in free_c_null_terminated_char_array()
    // TODO: replace by `out_vec.into_raw_parts()` once the API stabilizes
    let ptr = v.as_mut_ptr();
    mem::forget(v);

    (ptr, len)
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
    free_c_xdr_array(boxed.auth);
    free_c_xdr(boxed.result);
    free_c_xdr(boxed.transaction_data);
    free_c_xdr_array(boxed.events);
    free_c_xdr(boxed.pre_restore_transaction_data);
}

fn free_c_string(str: *mut libc::c_char) {
    if str.is_null() {
        return;
    }
    unsafe {
        _ = CString::from_raw(str);
    }
}

fn free_c_xdr(xdr: CXDR) {
    if xdr.xdr.is_null() {
        return;
    }
    unsafe {
        let _ = Vec::from_raw_parts(xdr.xdr, xdr.len, xdr.len);
    }
}

fn free_c_xdr_array(xdr_array: CXDRVector) {
    if xdr_array.array.is_null() {
        return;
    }
    unsafe {
        let v = Vec::from_raw_parts(xdr_array.array, xdr_array.len, xdr_array.len);
        for xdr in v {
            free_c_xdr(xdr);
        }
    }
}

fn from_c_string(str: *const libc::c_char) -> String {
    let c_str = unsafe { CStr::from_ptr(str) };
    c_str.to_str().unwrap().to_string()
}

fn from_c_xdr(xdr: CXDR) -> Vec<u8> {
    let s = unsafe { slice::from_raw_parts(xdr.xdr, xdr.len) };
    s.to_vec()
}

// Functions imported from Golang
extern "C" {
    // Free Strings returned from Go functions
    fn FreeGoXDR(xdr: CXDR);
    // LedgerKey XDR in base64 string to LedgerEntry XDR in base64 string
    fn SnapshotSourceGet(handle: libc::uintptr_t, ledger_key: CXDR) -> CXDR;
}

struct GoLedgerStorage {
    golang_handle: libc::uintptr_t,
    current_ledger_sequence: u32,
}

impl GoLedgerStorage {
    // Get the XDR, regardless of ttl
    fn get_xdr_internal(
        &self,
        key_xdr: &mut Vec<u8>,
    ) -> std::result::Result<Vec<u8>, ledger_storage::Error> {
        let key_c_xdr = CXDR {
            xdr: key_xdr.as_mut_ptr(),
            len: key_xdr.len(),
        };
        let res = unsafe { SnapshotSourceGet(self.golang_handle, key_c_xdr) };
        if res.xdr.is_null() {
            return Err(ledger_storage::Error::NotFound);
        }
        let v = from_c_xdr(res);
        unsafe { FreeGoXDR(res) };
        Ok(v)
    }
}

impl ledger_storage::LedgerGetter for GoLedgerStorage {
    fn get(
        &self,
        key: &LedgerKey,
        include_not_live: bool,
    ) -> std::result::Result<(LedgerEntry, Option<u32>), ledger_storage::Error> {
        let mut key_xdr = key.to_xdr(Limits::none())?;
        let xdr = self.get_xdr_internal(&mut key_xdr)?;

        let live_until_ledger_seq = match key {
            // TODO: it would probably be more efficient to do all of this in the Go side
            //       (e.g. it would allow us to query multiple entries at once)
            LedgerKey::ContractData(_) | LedgerKey::ContractCode(_) => {
                let key_hash: [u8; 32] = Sha256::digest(key_xdr).into();
                let ttl_key = LedgerKey::Ttl(LedgerKeyTtl {
                    key_hash: Hash(key_hash),
                });
                let mut ttl_key_xdr = ttl_key.to_xdr(Limits::none())?;
                let ttl_entry_xdr = self.get_xdr_internal(&mut ttl_key_xdr)?;
                let ttl_entry = LedgerEntry::from_xdr(ttl_entry_xdr, Limits::none())?;
                if let LedgerEntryData::Ttl(TtlEntry {
                    live_until_ledger_seq,
                    ..
                }) = ttl_entry.data
                {
                    Some(live_until_ledger_seq)
                } else {
                    return Err(ledger_storage::Error::UnexpectedLedgerEntryTypeForTtlKey {
                        ledger_entry_type: ttl_entry.data.name().to_string(),
                    });
                }
            }
            _ => None,
        };

        if !include_not_live
            && live_until_ledger_seq.is_some()
            && !is_live(live_until_ledger_seq.unwrap(), self.current_ledger_sequence)
        {
            return Err(ledger_storage::Error::NotLive);
        }

        let entry = LedgerEntry::from_xdr(xdr, Limits::none())?;
        Ok((entry, live_until_ledger_seq))
    }
}

pub(crate) fn is_live(live_until_ledger_seq: u32, current_ledger_seq: u32) -> bool {
    live_until_ledger_seq >= current_ledger_seq
}
