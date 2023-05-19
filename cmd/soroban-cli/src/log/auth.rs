use soroban_env_host::xdr::{
    ContractAuth, VecM,
};

pub fn auth(auth: &Vec<VecM<ContractAuth>>) {
    if !auth.is_empty() {
        tracing::debug!(?auth);
    }
}
