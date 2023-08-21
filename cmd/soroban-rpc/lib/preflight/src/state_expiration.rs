use soroban_env_host::xdr::ContractDataDurability::Persistent;
use soroban_env_host::xdr::{
    ContractCodeEntry, ContractDataDurability, ContractDataEntry, LedgerEntry, LedgerEntryData,
};
use std::convert::TryInto;

pub(crate) trait ExpirableLedgerEntry {
    fn durability(&self) -> ContractDataDurability;
    fn expiration_ledger_seq(&self) -> u32;
    fn has_expired(&self, current_ledger_seq: u32) -> bool {
        current_ledger_seq > self.expiration_ledger_seq()
    }
}

impl ExpirableLedgerEntry for &ContractCodeEntry {
    fn durability(&self) -> ContractDataDurability {
        Persistent
    }

    fn expiration_ledger_seq(&self) -> u32 {
        self.expiration_ledger_seq
    }
}

impl ExpirableLedgerEntry for &ContractDataEntry {
    fn durability(&self) -> ContractDataDurability {
        self.durability
    }

    fn expiration_ledger_seq(&self) -> u32 {
        self.expiration_ledger_seq
    }
}

impl<'a> TryInto<Box<dyn ExpirableLedgerEntry + 'a>> for &'a LedgerEntry {
    type Error = String;

    fn try_into(self) -> Result<Box<dyn ExpirableLedgerEntry + 'a>, Self::Error> {
        match &self.data {
            LedgerEntryData::ContractData(d) => Ok(Box::new(d)),
            LedgerEntryData::ContractCode(c) => Ok(Box::new(c)),
            _ => Err(format!(
                "Incorrect ledger entry type ({}) in footprint",
                self.data.name()
            )),
        }
    }
}

pub(crate) fn get_restored_ledger_sequence(
    current_ledger_seq: u32,
    min_persistent_entry_expiration: u32,
) -> u32 {
    return current_ledger_seq + min_persistent_entry_expiration - 1;
}

pub(crate) fn restore_ledger_entry(
    ledger_entry: &mut LedgerEntry,
    current_ledger_seq: u32,
    min_persistent_entry_expiration: u32,
) {
    let new_ledger_seq =
        get_restored_ledger_sequence(current_ledger_seq, min_persistent_entry_expiration);
    match &mut ledger_entry.data {
        LedgerEntryData::ContractData(d) => d.expiration_ledger_seq = new_ledger_seq,
        LedgerEntryData::ContractCode(c) => c.expiration_ledger_seq = new_ledger_seq,
        _ => (),
    }
}
