use soroban_env_host::xdr::ContractDataDurability::Persistent;
use soroban_env_host::xdr::{
    ContractCodeEntry, ContractDataDurability, ContractDataEntry, LedgerEntry, LedgerEntryData,
};
use std::convert::TryInto;

pub(crate) trait ExpirableLedgerEntry {
    fn durability(&self) -> ContractDataDurability;
    fn expiration_ledger_seq(&self) -> u32;
    fn has_expired(&self, current_ledger_seq: u32) -> bool {
        has_expired(current_ledger_seq, self.expiration_ledger_seq())
    }
}

impl ExpirableLedgerEntry for (&ContractCodeEntry, u32) {
    fn durability(&self) -> ContractDataDurability {
        Persistent
    }

    fn expiration_ledger_seq(&self) -> u32 {
        self.1
    }
}

impl ExpirableLedgerEntry for (&ContractDataEntry, u32) {
    fn durability(&self) -> ContractDataDurability {
        self.0.durability
    }

    fn expiration_ledger_seq(&self) -> u32 {
        self.1
    }
}

// Convert a ledger entry and its expiration into an ExpirableLedgerEntry
impl<'a> TryInto<Box<dyn ExpirableLedgerEntry + 'a>> for &'a (LedgerEntry, Option<u32>) {
    type Error = String;

    fn try_into(self) -> Result<Box<dyn ExpirableLedgerEntry + 'a>, Self::Error> {
        match (&self.0.data, self.1) {
            (LedgerEntryData::ContractData(d), Some(expiration_seq)) => {
                Ok(Box::new((d, expiration_seq)))
            }
            (LedgerEntryData::ContractCode(c), Some(expiration_seq)) => {
                Ok(Box::new((c, expiration_seq)))
            }
            (LedgerEntryData::ContractData(_) | LedgerEntryData::ContractCode(_), _) => {
                Err(format!(
                    "missing expiration for expirable ledger entry ({})",
                    self.0.data.name()
                ))
            }
            _ => Err(format!(
                "ledger entry type ({}) is not expirable",
                self.0.data.name()
            )),
        }
    }
}

pub(crate) fn has_expired(expiration_ledger_seq: u32, current_ledger_seq: u32) -> bool {
    current_ledger_seq > expiration_ledger_seq
}

pub(crate) fn get_restored_ledger_sequence(
    current_ledger_seq: u32,
    min_persistent_entry_expiration: u32,
) -> u32 {
    return current_ledger_seq + min_persistent_entry_expiration - 1;
}
