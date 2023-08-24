pub fn txn_error(error: &crate::rpc::Error) {
    tracing::debug!(?error);
}
