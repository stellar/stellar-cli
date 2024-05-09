use std::fmt::{Display, Formatter};

use soroban_env_host::xdr::{Limits, Transaction, WriteXdr};

pub enum TxnResult<R> {
    Txn(Transaction),
    Res(R),
}

impl<R> TxnResult<R> {
    pub fn into_result(self) -> Option<R> {
        match self {
            TxnResult::Res(res) => Some(res),
            TxnResult::Txn(_) => None,
        }
    }
}

impl<V> Display for TxnResult<V>
where
    V: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TxnResult::Txn(tx) => write!(
                f,
                "{}",
                tx.to_xdr_base64(Limits::none())
                    .map_err(|_| std::fmt::Error)?
            ),
            TxnResult::Res(value) => write!(f, "{value}"),
        }
    }
}
