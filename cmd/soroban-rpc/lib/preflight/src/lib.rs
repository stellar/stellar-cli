extern crate libc;
extern crate sha2;
extern crate soroban_env_host;

use sha2::{Digest, Sha256};
use soroban_env_host::auth::RecordedAuthPayload;
use soroban_env_host::budget::Budget;
use soroban_env_host::events::{Event, Events};
use soroban_env_host::fees::{
    compute_transaction_resource_fee, FeeConfiguration, TransactionResources,
};
use soroban_env_host::storage::{AccessType, Footprint, SnapshotSource, Storage, StorageMap};
use soroban_env_host::xdr::{
    self, AccountId, AddressWithNonce, ContractAuth, DecoratedSignature, DiagnosticEvent,
    ExtensionPoint, InvokeHostFunctionOp, LedgerEntry, LedgerKey, Memo, MuxedAccount,
    MuxedAccountMed25519, Operation, OperationBody, Preconditions, ReadXdr, ScHostStorageErrorCode,
    ScStatus,
    ScUnknownErrorCode::{General, Xdr},
    ScVal, SequenceNumber, SignatureHint, SorobanResources, SorobanTransactionData, Transaction,
    TransactionExt, TransactionV1Envelope, Uint256, WriteXdr,
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

#[repr(C)]
pub struct CPreflightResult {
    pub error: *mut libc::c_char, // Error string in case of error, otherwise null
    pub results: *mut *mut libc::c_char, // NULL terminated array of XDR SCVals in base64
    pub transaction_data: *mut libc::c_char, // SorobanTransactionData XDR in base64
    pub min_fee: i64,             // Minimum recommended resource fee
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
        results: null_mut(),
        transaction_data: null_mut(),
        min_fee: 0,
        auth: null_mut(),
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
    let src = Rc::new(CSnapshotSource { handle });
    let storage = Storage::with_recording_footprint(src);
    let budget = Budget::default();
    let host = Host::with_storage_and_budget(storage, budget);

    host.switch_to_recording_auth();
    host.set_source_account(source_account);
    host.set_ledger_info(ledger_info.into());

    // Run the preflight.
    let results = host.invoke_functions(invoke_hf_op.clone().functions.into())?;
    let auth_payloads = host.get_recorded_auth_payloads()?;

    // Recover, convert and return the storage footprint and other values to C.
    let (storage, budget, events) = host.try_finish().unwrap();

    let diagnostic_events = host_events_to_diagnostic_events(&events)?;
    // TODO: add the auth info to invoke_hf_op so that it's taken into account when estimating the
    //       transaction size
    let (transaction_data, min_fee) = compute_transaction_data_and_min_fee(
        &invoke_hf_op,
        &CSnapshotSource { handle },
        &storage,
        &budget,
        &diagnostic_events,
    )?;
    let transaction_data_cstr = CString::new(transaction_data.to_xdr_base64()?)?;
    Ok(CPreflightResult {
        error: null_mut(),
        results: scvals_to_c(results)?,
        transaction_data: transaction_data_cstr.into_raw(),
        min_fee: min_fee,
        auth: recorded_auth_payloads_to_c(auth_payloads)?,
        events: diagnostic_events_to_c(diagnostic_events)?,
        cpu_instructions: budget.get_cpu_insns_count(),
        memory_bytes: budget.get_mem_bytes_count(),
    })
}

fn compute_transaction_data_and_min_fee(
    invoke_hf_op: &InvokeHostFunctionOp,
    snapshot_source: &CSnapshotSource,
    storage: &Storage,
    budget: &Budget,
    events: &Vec<DiagnosticEvent>,
) -> Result<(SorobanTransactionData, i64), Box<dyn error::Error>> {
    let soroban_resources = calculate_soroban_resources(snapshot_source, storage, budget, events)?;
    let fee_configuration = get_fee_configuration(snapshot_source)?;

    let read_write_entries = soroban_resources.footprint.read_write.as_vec().len() as u32;

    let transaction_resources = TransactionResources {
        instructions: soroban_resources.instructions,
        read_entries: soroban_resources.footprint.read_only.as_vec().len() as u32
            + read_write_entries,
        write_entries: read_write_entries,
        read_bytes: soroban_resources.read_bytes,
        write_bytes: soroban_resources.write_bytes,
        metadata_size_bytes: soroban_resources.extended_meta_data_size_bytes,
        // Note: we could get a better transaction size if the full transaction was passed down to libpreflight
        transaction_size_bytes: estimate_max_transaction_size(
            invoke_hf_op,
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

fn estimate_max_transaction_size(
    invoke_hf_op: &InvokeHostFunctionOp,
    fp: &LedgerFootprint,
) -> Result<u32, Box<dyn error::Error>> {
    let source = MuxedAccount::MuxedEd25519(MuxedAccountMed25519 {
        id: 0,
        ed25519: Uint256([0; 32]),
    });
    // generate the maximum memo size and signature size
    // TODO: is this being too conservative?
    let memo_text: Vec<u8> = [0; 28].into();
    // TODO: find a better way to do this:
    let mut signatures: Vec<DecoratedSignature> = vec![];
    let mut signatures_left = 20;
    while signatures_left > 0 {
        signatures.push(DecoratedSignature {
            hint: SignatureHint([0; 4]),
            signature: Default::default(),
        });
        signatures_left -= 1;
    }
    let envelope = TransactionV1Envelope {
        tx: Transaction {
            source_account: source.clone(),
            fee: 0,
            seq_num: SequenceNumber(0),
            cond: Preconditions::None,
            memo: Memo::Text(memo_text.try_into()?),
            operations: vec![Operation {
                source_account: Some(source),
                body: OperationBody::InvokeHostFunction(invoke_hf_op.clone()),
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
    Ok(envelope_size as u32)
}

fn calculate_soroban_resources(
    snapshot_source: &CSnapshotSource,
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
        calculate_unmodified_ledger_entry_bytes(&fp.read_write.as_vec(), snapshot_source)?;
    let read_bytes =
        calculate_unmodified_ledger_entry_bytes(&fp.read_only.as_vec(), snapshot_source)?
            + original_write_ledger_entry_bytes;
    let write_bytes =
        calculate_modified_read_write_ledger_entry_bytes(&storage.footprint, &storage.map, budget)?;
    let meta_data_size_bytes =
        original_write_ledger_entry_bytes + write_bytes + calculate_event_size_bytes(events)?;

    // Add a 30% leeway
    let instructions = budget.get_cpu_insns_count() * 13 / 10;
    Ok(SorobanResources {
        footprint: fp,
        instructions: instructions as u32,
        read_bytes: read_bytes,
        write_bytes: write_bytes,
        extended_meta_data_size_bytes: meta_data_size_bytes,
    })
}

fn get_fee_configuration(
    _snapshot_source: &CSnapshotSource,
) -> Result<FeeConfiguration, Box<dyn error::Error>> {
    // TODO: (at least part of) these values should be obtained from the network's ConfigSetting LedgerEntries
    //       (instead of hardcoding them to the initial values in the network)

    // Taken from Stellar Core's InitialSorobanNetworkConfig in NetworkConfig.h
    Ok(FeeConfiguration {
        fee_per_instruction_increment: 100,
        fee_per_read_entry: 5000,
        fee_per_write_entry: 20000,
        fee_per_read_1kb: 1000,
        fee_per_write_1kb: 4000,
        fee_per_historical_1kb: 100,
        fee_per_metadata_1kb: 200,
        fee_per_propagate_1kb: 2000,
    })
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
                    res += (entry_bytes.len() + key_bytes.len()) as u32;
                }
            }
            // TODO: turn this panic into an error
            None => panic!("ledger entry not in footprint"),
        }
    }
    Ok(res)
}

fn calculate_unmodified_ledger_entry_bytes(
    ledger_entries: &Vec<LedgerKey>,
    snapshot_source: &CSnapshotSource,
) -> Result<u32, Box<dyn error::Error>> {
    let mut res: u32 = 0;
    for lk in ledger_entries {
        res += lk.to_xdr()?.len() as u32;
        // TODO: remove unnecessary Rc
        match snapshot_source.get(&Rc::new(lk.clone())) {
            Ok(le) => {
                let entry_bytes = le.to_xdr()?;
                res += entry_bytes.len() as u32;
            }
            Err(e) => {
                if e.status == ScHostStorageErrorCode::AccessToUnknownEntry.into() {
                    // The entry is not present in the unmodified ledger storage.
                    // We assume it to be due to the entry being created by a host function invocation.
                    // Thus, we shouldn't count it in as unmodified.
                    continue;
                }
                return Err(e)?;
            }
        };
    }
    Ok(res)
}

fn calculate_event_size_bytes(events: &Vec<DiagnosticEvent>) -> Result<u32, xdr::Error> {
    let mut res: u32 = 0;
    for e in events {
        let event_xdr = e.to_xdr()?;
        res += event_xdr.len() as u32;
    }
    Ok(res)
}

fn scvals_to_c(scvals: Vec<ScVal>) -> Result<*mut *mut libc::c_char, Box<dyn error::Error>> {
    let xdr_base64_vec: Vec<String> = scvals
        .iter()
        .map(|v| v.to_xdr_base64())
        .collect::<Result<Vec<_>, _>>()?;
    string_vec_to_c_to_null_terminated_char_array(xdr_base64_vec)
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
        if !(*result).results.is_null() {
            free_c_null_terminated_char_array((*result).results);
        }
        if !(*result).transaction_data.is_null() {
            let _ = CString::from_raw((*result).transaction_data);
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
