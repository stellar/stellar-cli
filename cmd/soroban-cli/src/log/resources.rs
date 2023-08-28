use soroban_env_host::xdr::SorobanResources;

pub fn resources(resources: &SorobanResources) {
    tracing::debug!(?resources);
}
