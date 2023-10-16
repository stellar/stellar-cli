use soroban_env_host::xdr::ContractDataDurability::Persistent;
use soroban_env_host::xdr::{
    ContractCodeEntry, ContractDataDurability, ContractDataEntry, LedgerEntry, LedgerEntryData,
};
use std::convert::TryInto;

pub(crate) trait TTLLedgerEntry {
    fn durability(&self) -> ContractDataDurability;
    fn live_until_ledger_seq(&self) -> u32;
    fn is_live(&self, current_ledger_seq: u32) -> bool {
        is_live(self.live_until_ledger_seq(), current_ledger_seq)
    }
}

impl TTLLedgerEntry for (&ContractCodeEntry, u32) {
    fn durability(&self) -> ContractDataDurability {
        Persistent
    }

    fn live_until_ledger_seq(&self) -> u32 {
        self.1
    }
}

impl TTLLedgerEntry for (&ContractDataEntry, u32) {
    fn durability(&self) -> ContractDataDurability {
        self.0.durability
    }

    fn live_until_ledger_seq(&self) -> u32 {
        self.1
    }
}

// Convert a ledger entry and its Time to live (i.e. live_until_seq) into a TTLLedgerEntry
impl<'a> TryInto<Box<dyn TTLLedgerEntry + 'a>> for &'a (LedgerEntry, Option<u32>) {
    type Error = String;

    fn try_into(self) -> Result<Box<dyn TTLLedgerEntry + 'a>, Self::Error> {
        match (&self.0.data, self.1) {
            (LedgerEntryData::ContractData(d), Some(live_until_seq)) => {
                Ok(Box::new((d, live_until_seq)))
            }
            (LedgerEntryData::ContractCode(c), Some(live_until_seq)) => {
                Ok(Box::new((c, live_until_seq)))
            }
            (LedgerEntryData::ContractData(_) | LedgerEntryData::ContractCode(_), _) => Err(
                format!("missing ttl for ledger entry ({})", self.0.data.name()),
            ),
            _ => Err(format!(
                "ledger entry type ({}) cannot have a TTL",
                self.0.data.name()
            )),
        }
    }
}

pub(crate) fn is_live(live_until_ledger_seq: u32, current_ledger_seq: u32) -> bool {
    live_until_ledger_seq >= current_ledger_seq
}

pub(crate) fn get_restored_ledger_sequence(
    current_ledger_seq: u32,
    min_persistent_ttl: u32,
) -> u32 {
    current_ledger_seq + min_persistent_ttl - 1
}
