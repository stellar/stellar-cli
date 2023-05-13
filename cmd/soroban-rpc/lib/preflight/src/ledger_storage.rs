use base64::DecodeError;
use soroban_env_host::storage::SnapshotSource;
use soroban_env_host::xdr::ScUnknownErrorCode::{General, Xdr};
use soroban_env_host::xdr::{
    Error as XdrError, LedgerEntry, LedgerKey, ReadXdr, ScHostStorageErrorCode, ScStatus, WriteXdr,
};
use soroban_env_host::HostError;
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
    ) -> *const libc::c_char;
    // TODO: this function is unnecessary, we can just look for null in SnapshotSourceGet
    // LedgerKey XDR in base64 string to bool
    fn SnapshotSourceHas(handle: libc::uintptr_t, ledger_key: *const libc::c_char) -> libc::c_int;
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
}

impl Error {
    fn to_host_error(&self) -> HostError {
        match self {
            Error::NotFound => HostError::from(ScHostStorageErrorCode::AccessToUnknownEntry),
            Error::Xdr(_) => ScStatus::UnknownError(Xdr).into(),
            _ => ScStatus::UnknownError(General).into(),
        }
    }
}

pub(crate) struct LedgerStorage {
    pub(crate) golang_handle: libc::uintptr_t,
}

impl LedgerStorage {
    fn get_xdr_base64(&self, key: &LedgerKey) -> Result<String, Error> {
        let key_xdr = key.to_xdr_base64()?;
        let key_cstr = CString::new(key_xdr)?;
        let res = unsafe { SnapshotSourceGet(self.golang_handle, key_cstr.as_ptr()) };
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

    pub fn get_xdr(&self, key: &LedgerKey) -> Result<Vec<u8>, Error> {
        let base64_str = self.get_xdr_base64(key)?;
        Ok(base64::decode(base64_str)?)
    }
}

impl SnapshotSource for LedgerStorage {
    fn get(&self, key: &Rc<LedgerKey>) -> Result<Rc<LedgerEntry>, HostError> {
        let base64_str = self
            .get_xdr_base64(key)
            .map_err(|e| Error::to_host_error(&e))?;
        let entry =
            LedgerEntry::from_xdr_base64(base64_str).map_err(|_| ScStatus::UnknownError(Xdr))?;
        Ok(entry.into())
    }

    fn has(&self, key: &Rc<LedgerKey>) -> Result<bool, HostError> {
        let key_xdr = key
            .to_xdr_base64()
            .map_err(|_| ScStatus::UnknownError(Xdr))?;
        let key_cstr = CString::new(key_xdr).map_err(|_| ScStatus::UnknownError(Xdr))?;
        let res = unsafe { SnapshotSourceHas(self.golang_handle, key_cstr.as_ptr()) };
        Ok(res != 0)
    }
}
