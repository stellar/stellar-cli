use std::fmt::Display;

/// A barebones implementation of Total Order IDs (TOIDs) from
/// [SEP-35](https://stellar.org/protocol/sep-35), using the reference
/// implementation from the Go
/// [`stellar/go/toid`](https://github.com/stellar/go/blob/b4ba6f8e67f274bf84d21b0effb01ea8a914b766/toid/main.go#L8-L56)
/// package.
#[derive(Copy, Clone)]
pub struct Toid {
    ledger_sequence: u32,
    transaction_order: u32,
    operation_order: u32,
}

const LEDGER_MASK: u64 = (1 << 32) - 1;
const TRANSACTION_MASK: u64 = (1 << 20) - 1;
const OPERATION_MASK: u64 = (1 << 12) - 1;
const LEDGER_SHIFT: u64 = 32;
const TRANSACTION_SHIFT: u64 = 12;
const OPERATION_SHIFT: u64 = 0;

impl Toid {
    pub fn new(ledger: u32, tx_order: u32, op_order: u32) -> Toid {
        Toid {
            ledger_sequence: ledger,
            transaction_order: tx_order,
            operation_order: op_order,
        }
    }

    pub fn to_paging_token(self) -> String {
        let u: u64 = self.into();
        format!("{u:019}")
    }
}

impl From<u64> for Toid {
    fn from(item: u64) -> Self {
        let ledger: u32 = ((item & LEDGER_MASK) >> LEDGER_SHIFT).try_into().unwrap();
        let tx_order: u32 = ((item & TRANSACTION_MASK) >> TRANSACTION_SHIFT)
            .try_into()
            .unwrap();
        let op_order: u32 = ((item & OPERATION_MASK) >> OPERATION_SHIFT)
            .try_into()
            .unwrap();

        Toid::new(ledger, tx_order, op_order)
    }
}

impl From<Toid> for u64 {
    fn from(item: Toid) -> Self {
        let l: u64 = item.ledger_sequence.into();
        let t: u64 = item.transaction_order.into();
        let o: u64 = item.operation_order.into();

        let mut result: u64 = 0;
        result |= (l & LEDGER_MASK) << LEDGER_SHIFT;
        result |= (t & TRANSACTION_MASK) << TRANSACTION_SHIFT;
        result |= (o & OPERATION_MASK) << OPERATION_SHIFT;

        result
    }
}

impl Display for Toid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let u: u64 = (*self).into();
        write!(f, "{u}")
    }
}
