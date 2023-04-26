use soroban_env_host::xdr::LedgerFootprint;

pub fn footprint(footprint: &LedgerFootprint) {
    tracing::debug!(?footprint);
}
