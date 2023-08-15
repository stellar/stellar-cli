use base64::{engine::general_purpose::STANDARD as base64, DecodeError, Engine as _};
use soroban_env_host::storage::SnapshotSource;
use soroban_env_host::xdr::ContractDataDurability::Persistent;
use soroban_env_host::xdr::{
    ConfigSettingEntry, ConfigSettingId, Error as XdrError, LedgerEntry, LedgerEntryData,
    LedgerKey, LedgerKeyConfigSetting, ReadXdr, ScError, ScErrorCode, WriteXdr,
};
use soroban_env_host::HostError;
use state_expiration::{restore_ledger_entry, ExpirableLedgerEntry};
use std::cell::RefCell;
use std::collections::HashSet;
use std::convert::TryInto;
use std::ffi::{CStr, CString, NulError};
use std::rc::Rc;
use std::str::Utf8Error;

// Functions imported from Golang
extern "C" {
    // Free Strings returned from Go functions
    fn FreeGoCString(str: *const libc::c_char);
    // LedgerKey XDR in base64 string to LedgerEntry XDR in base64 string
    fn SnapshotSourceGet(
        handle: libc::uintptr_t,
        ledger_key: *const libc::c_char,
        include_expired: libc::c_int,
    ) -> *const libc::c_char;
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("not found")]
    NotFound,
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("nul error: {0}")]
    NulError(#[from] NulError),
    #[error("decode error: {0}")]
    DecodeError(#[from] DecodeError),
    #[error("utf8 error: {0}")]
    Utf8Error(#[from] Utf8Error),
    #[error("unexpected config ledger entry for setting_id {setting_id}")]
    UnexpectedConfigLedgerEntry { setting_id: String },
}

impl Error {
    fn to_host_error(&self) -> HostError {
        match self {
            Error::NotFound => ScError::Storage(ScErrorCode::MissingValue).into(),
            Error::Xdr(_) => ScError::Value(ScErrorCode::InvalidInput).into(),
            _ => ScError::Context(ScErrorCode::InternalError).into(),
        }
    }
}

struct EntryRestoreTracker {
    current_ledger_seq: u32,
    min_persistent_entry_expiration: u32,
    // RefCell is needed to mutate the hashset inside SnapshotSource::get(), which is an immutable method
    ledger_keys_requiring_restore: RefCell<HashSet<LedgerKey>>,
}

impl EntryRestoreTracker {
    pub(crate) fn track_and_restore(&self, key: &LedgerKey, entry: &mut LedgerEntry) {
        if self.track(key, entry) {
            restore_ledger_entry(
                entry,
                self.current_ledger_seq,
                self.min_persistent_entry_expiration,
            );
        }
    }

    pub(crate) fn track(&self, key: &LedgerKey, entry: &LedgerEntry) -> bool {
        let expirable_entry: Box<dyn ExpirableLedgerEntry> = match entry.try_into() {
            Ok(e) => e,
            Err(_) => {
                // Nothing to track, the entry isn't expirable
                return false;
            }
        };
        if expirable_entry.durability() != Persistent
            || !expirable_entry.has_expired(self.current_ledger_seq)
        {
            // Nothing to track, the entry isn't persistent (and thus not restorable) or
            // it hasn't expired
            return false;
        }
        println!("succesfully tracking {:?}", key);
        self.ledger_keys_requiring_restore
            .borrow_mut()
            .insert(key.clone());
        return true;
    }
}

pub(crate) struct LedgerStorage {
    golang_handle: libc::uintptr_t,
    restore_tracker: Option<EntryRestoreTracker>,
}

impl LedgerStorage {
    pub(crate) fn new(golang_handle: libc::uintptr_t) -> Self {
        LedgerStorage {
            golang_handle,
            restore_tracker: None,
        }
    }

    pub(crate) fn with_restore_tracking(
        golang_handle: libc::uintptr_t,
        current_ledger_sequence: u32,
    ) -> Result<Self, Error> {
        // First, we initialize it without the tracker, to get the minimum restore ledger from the network
        let mut ledger_storage = LedgerStorage {
            golang_handle,
            restore_tracker: None,
        };
        let setting_id = ConfigSettingId::StateExpiration;
        let ConfigSettingEntry::StateExpiration(state_expiration) =
            ledger_storage.get_configuration_setting(setting_id)?
            else {
                return Err(
                    Error::UnexpectedConfigLedgerEntry { setting_id: setting_id.name().to_string() }
                );
            };
        // Now that we have the state expiration config, we can build the tracker
        ledger_storage.restore_tracker = Some(EntryRestoreTracker {
            current_ledger_seq: current_ledger_sequence,
            ledger_keys_requiring_restore: RefCell::new(HashSet::new()),
            min_persistent_entry_expiration: state_expiration.min_persistent_entry_expiration,
        });
        return Ok(ledger_storage);
    }

    fn get_xdr_base64(&self, key: &LedgerKey, include_expired: bool) -> Result<String, Error> {
        let key_xdr = key.to_xdr_base64()?;
        let key_cstr = CString::new(key_xdr)?;
        let res = unsafe {
            SnapshotSourceGet(
                self.golang_handle,
                key_cstr.as_ptr(),
                include_expired.into(),
            )
        };
        if res.is_null() {
            return Err(Error::NotFound);
        }
        let unsafe_cstr = unsafe { CStr::from_ptr(res) };
        // Make a safe copy of the string before freeing it
        // Note: If we wanted more performance we should create an unsafe version of get_xdr_base64() which
        //       returned a view of the C buffer as follows:
        // return Ok((res, unsafe_cstr.to_bytes())); // we would later need to call FreeGoCString(res)
        let str = String::from_utf8_lossy(unsafe_cstr.to_bytes()).to_string();
        unsafe { FreeGoCString(res) };
        Ok(str)
    }

    pub(crate) fn get(&self, key: &LedgerKey, include_expired: bool) -> Result<LedgerEntry, Error> {
        let base64_str = self.get_xdr_base64(key, include_expired)?;
        let entry = LedgerEntry::from_xdr_base64(base64_str)?;
        Ok(entry)
    }

    pub(crate) fn get_xdr(&self, key: &LedgerKey, include_expired: bool) -> Result<Vec<u8>, Error> {
        let base64_str = self.get_xdr_base64(key, include_expired)?;
        Ok(base64.decode(base64_str)?)
    }

    pub(crate) fn get_configuration_setting(
        &self,
        setting_id: ConfigSettingId,
    ) -> Result<ConfigSettingEntry, Error> {
        let key = LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
            config_setting_id: setting_id,
        });
        match self.get(&key, false)? {
            LedgerEntry {
                data: LedgerEntryData::ConfigSetting(cs),
                ..
            } => Ok(cs),
            _ => Err(Error::UnexpectedConfigLedgerEntry {
                setting_id: setting_id.name().to_string(),
            }),
        }
    }

    pub(crate) fn get_ledger_keys_requiring_restore(&self) -> HashSet<LedgerKey> {
        match self.restore_tracker {
            Some(ref t) => t.ledger_keys_requiring_restore.borrow().clone(),
            None => HashSet::new(),
        }
    }
}

impl SnapshotSource for LedgerStorage {
    fn get(&self, key: &Rc<LedgerKey>) -> Result<Rc<LedgerEntry>, HostError> {
        let mut entry = <LedgerStorage>::get(self, key, self.restore_tracker.is_some())
            .map_err(|e| Error::to_host_error(&e))?;
        if let Some(ref tracker) = self.restore_tracker {
            // If the entry expired, we modify it to make it seem like it was restored
            tracker.track_and_restore(key, &mut entry);
        }
        Ok(entry.into())
    }

    fn has(&self, key: &Rc<LedgerKey>) -> Result<bool, HostError> {
        let entry = match <LedgerStorage>::get(self, key, self.restore_tracker.is_some()) {
            Err(e) => match e {
                Error::NotFound => return Ok(false),
                _ => return Err(Error::to_host_error(&e)),
            },
            Ok(le) => le,
        };
        if let Some(ref tracker) = self.restore_tracker {
            tracker.track(key, &entry);
        }
        Ok(true)
    }
}
