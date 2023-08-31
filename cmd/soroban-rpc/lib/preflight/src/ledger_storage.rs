use sha2::Digest;
use soroban_env_host::storage::SnapshotSource;
use soroban_env_host::xdr::ContractDataDurability::Persistent;
use soroban_env_host::xdr::{
    ConfigSettingEntry, ConfigSettingId, Error as XdrError, ExpirationEntry, Hash, LedgerEntry,
    LedgerEntryData, LedgerKey, LedgerKeyConfigSetting, LedgerKeyExpiration, ReadXdr, ScError,
    ScErrorCode, WriteXdr,
};
use soroban_env_host::HostError;
use state_expiration::{get_restored_ledger_sequence, has_expired, ExpirableLedgerEntry};
use std::cell::RefCell;
use std::collections::HashSet;
use std::convert::TryInto;
use std::ffi::NulError;
use std::rc::Rc;
use std::str::Utf8Error;
use {from_c_xdr, CXDR};

// Functions imported from Golang
extern "C" {
    // Free Strings returned from Go functions
    fn FreeGoXDR(xdr: CXDR);
    // LedgerKey XDR in base64 string to LedgerEntry XDR in base64 string
    fn SnapshotSourceGet(handle: libc::uintptr_t, ledger_key: CXDR) -> CXDR;
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("not found")]
    NotFound,
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("nul error: {0}")]
    NulError(#[from] NulError),
    #[error("utf8 error: {0}")]
    Utf8Error(#[from] Utf8Error),
    #[error("unexpected config ledger entry for setting_id {setting_id}")]
    UnexpectedConfigLedgerEntry { setting_id: String },
    #[error("unexpected ledger entry type ({ledger_entry_type}) for expiration ledger key")]
    UnexpectedLedgerEntryTypeForExpirationKey { ledger_entry_type: String },
}

impl From<Error> for HostError {
    fn from(value: Error) -> Self {
        match value {
            Error::NotFound => ScError::Storage(ScErrorCode::MissingValue).into(),
            Error::Xdr(_) => ScError::Value(ScErrorCode::InvalidInput).into(),
            _ => ScError::Context(ScErrorCode::InternalError).into(),
        }
    }
}

struct EntryRestoreTracker {
    min_persistent_entry_expiration: u32,
    // RefCell is needed to mutate the hashset inside SnapshotSource::get(), which is an immutable method
    ledger_keys_requiring_restore: RefCell<HashSet<LedgerKey>>,
}

impl EntryRestoreTracker {
    // Tracks ledger entries which need to be restored and returns its expiration as it was restored
    pub(crate) fn track_and_restore(
        &self,
        current_ledger_sequence: u32,
        key: &LedgerKey,
        entry_and_expiration: &(LedgerEntry, Option<u32>),
    ) -> Option<u32> {
        let expirable_entry: Box<dyn ExpirableLedgerEntry> = match entry_and_expiration.try_into() {
            Ok(e) => e,
            Err(_) => {
                // Nothing to track, the entry isn't expirable
                return None;
            }
        };
        if expirable_entry.durability() != Persistent
            || !expirable_entry.has_expired(current_ledger_sequence)
        {
            // Nothing to track, the entry isn't persistent (and thus not restorable) or
            // it hasn't expired
            return Some(expirable_entry.expiration_ledger_seq());
        }
        self.ledger_keys_requiring_restore
            .borrow_mut()
            .insert(key.clone());
        Some(get_restored_ledger_sequence(
            current_ledger_sequence,
            self.min_persistent_entry_expiration,
        ))
    }
}

pub(crate) struct LedgerStorage {
    golang_handle: libc::uintptr_t,
    current_ledger_sequence: u32,
    restore_tracker: Option<EntryRestoreTracker>,
}

impl LedgerStorage {
    pub(crate) fn new(golang_handle: libc::uintptr_t, current_ledger_sequence: u32) -> Self {
        LedgerStorage {
            golang_handle,
            current_ledger_sequence,
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
            current_ledger_sequence,
            restore_tracker: None,
        };
        let setting_id = ConfigSettingId::StateExpiration;
        let ConfigSettingEntry::StateExpiration(state_expiration) =
            ledger_storage.get_configuration_setting(setting_id)?
        else {
            return Err(Error::UnexpectedConfigLedgerEntry {
                setting_id: setting_id.name().to_string(),
            });
        };
        // Now that we have the state expiration config, we can build the tracker
        ledger_storage.restore_tracker = Some(EntryRestoreTracker {
            ledger_keys_requiring_restore: RefCell::new(HashSet::new()),
            min_persistent_entry_expiration: state_expiration.min_persistent_entry_expiration,
        });
        Ok(ledger_storage)
    }

    // Get the XDR, regardless of expiration
    fn get_xdr_internal(&self, key_xdr: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
        let key_c_xdr = CXDR {
            xdr: key_xdr.as_mut_ptr(),
            len: key_xdr.len(),
        };
        let res = unsafe { SnapshotSourceGet(self.golang_handle, key_c_xdr) };
        if res.xdr.is_null() {
            return Err(Error::NotFound);
        }
        let v = from_c_xdr(res.clone());
        unsafe { FreeGoXDR(res) };
        Ok(v)
    }

    pub(crate) fn get(
        &self,
        key: &LedgerKey,
        include_expired: bool,
    ) -> Result<(LedgerEntry, Option<u32>), Error> {
        let mut key_xdr = key.to_xdr()?;
        let xdr = self.get_xdr_internal(&mut key_xdr)?;

        let expiration_seq = match key {
            LedgerKey::ContractData(_) | LedgerKey::ContractCode(_) => {
                let key_hash: [u8; 32] = sha2::Sha256::digest(key_xdr).into();
                let expiration_key = LedgerKey::Expiration(LedgerKeyExpiration {
                    key_hash: Hash(key_hash),
                });
                let mut expiration_key_xdr = expiration_key.to_xdr()?;
                let expiration_entry_xdr = self.get_xdr_internal(&mut expiration_key_xdr)?;
                let expiration_entry = LedgerEntry::from_xdr(expiration_entry_xdr)?;
                if let LedgerEntryData::Expiration(ExpirationEntry {
                    expiration_ledger_seq: expiration_ledger_seq,
                    ..
                }) = expiration_entry.data
                {
                    Some(expiration_ledger_seq)
                } else {
                    return Err(Error::UnexpectedLedgerEntryTypeForExpirationKey {
                        ledger_entry_type: expiration_entry.data.name().to_string(),
                    });
                }
            }
            _ => None,
        };

        if !include_expired
            && expiration_seq.is_some()
            && has_expired(expiration_seq.unwrap(), self.current_ledger_sequence)
        {
            return Err(Error::NotFound);
        }

        let entry = LedgerEntry::from_xdr(xdr)?;
        Ok((entry, expiration_seq))
    }

    pub(crate) fn get_xdr(&self, key: &LedgerKey, include_expired: bool) -> Result<Vec<u8>, Error> {
        // TODO: this can be optimized since for entry types other than ContractCode/ContractData,
        //       they don't need to be deserialized and serialized again
        let (entry, _) = self.get(key, include_expired)?;
        Ok(entry.to_xdr()?)
    }

    pub(crate) fn get_configuration_setting(
        &self,
        setting_id: ConfigSettingId,
    ) -> Result<ConfigSettingEntry, Error> {
        let key = LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
            config_setting_id: setting_id,
        });
        match self.get(&key, false)? {
            (
                LedgerEntry {
                    data: LedgerEntryData::ConfigSetting(cs),
                    ..
                },
                _,
            ) => Ok(cs),
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
    fn get(&self, key: &Rc<LedgerKey>) -> Result<(Rc<LedgerEntry>, Option<u32>), HostError> {
        let mut entry_and_expiration =
            <LedgerStorage>::get(self, key, self.restore_tracker.is_some())?;
        if let Some(ref tracker) = self.restore_tracker {
            // If the entry expired, we modify the expiration to make it seem like it was restored
            entry_and_expiration.1 =
                tracker.track_and_restore(self.current_ledger_sequence, key, &entry_and_expiration);
        }
        Ok((entry_and_expiration.0.into(), entry_and_expiration.1))
    }

    fn has(&self, key: &Rc<LedgerKey>) -> Result<bool, HostError> {
        let entry_and_expiration =
            match <LedgerStorage>::get(self, key, self.restore_tracker.is_some()) {
                Err(e) => match e {
                    Error::NotFound => return Ok(false),
                    _ => return Err(HostError::from(e)),
                },
                Ok(le) => le,
            };
        if let Some(ref tracker) = self.restore_tracker {
            _ = tracker.track_and_restore(self.current_ledger_sequence, key, &entry_and_expiration);
        }
        Ok(true)
    }
}
