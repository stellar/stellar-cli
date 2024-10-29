use crate::xdr::LedgerFootprint;

pub fn footprint(footprint: &LedgerFootprint) {
    tracing::debug!("{footprint:#?}");
}
