use soroban_env_host::xdr::Transaction;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
