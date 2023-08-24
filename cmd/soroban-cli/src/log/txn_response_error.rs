use crate::rpc::GetTransactionResponse;

pub fn txn_response_error(error: &GetTransactionResponse) {
    tracing::debug!(?error);
}
