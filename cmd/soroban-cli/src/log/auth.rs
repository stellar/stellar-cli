use soroban_env_host::xdr::{SorobanAuthorizationEntry, VecM};

pub fn auth(auth: &Vec<VecM<SorobanAuthorizationEntry>>) {
    if !auth.is_empty() {
        tracing::debug!(?auth);
    }
}
