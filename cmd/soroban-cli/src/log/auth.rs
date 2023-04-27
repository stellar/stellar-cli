use soroban_env_host::xdr::ContractAuth;

pub fn auth(auth: &[ContractAuth]) {
    if !auth.is_empty() {
        tracing::debug!(?auth);
    }
}
