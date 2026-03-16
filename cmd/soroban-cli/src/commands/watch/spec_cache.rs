use std::collections::HashMap;

use soroban_spec_tools::{event::DecodedEvent, Spec};

use crate::config::{locator, network};
use crate::get_spec::get_remote_contract_spec;
use crate::xdr::{Limits, ReadXdr, ScSpecEntry, ScVal};

pub type SpecCache = HashMap<String, Option<Spec>>;

/// Fetch raw spec entries for a contract. Returns `None` if the spec can't be fetched.
pub(super) async fn fetch_spec_entries(
    contract_id_str: &str,
    locator: &locator::Args,
    network_args: &network::Args,
) -> Option<Vec<ScSpecEntry>> {
    let contract_id = stellar_strkey::Contract::from_string(contract_id_str)
        .inspect_err(|e| tracing::debug!("Invalid contract ID {contract_id_str}: {e}"))
        .ok()?;
    get_remote_contract_spec(&contract_id.0, locator, network_args, None, None)
        .await
        .inspect_err(|e| tracing::debug!("Failed to fetch spec for {contract_id_str}: {e}"))
        .ok()
}

pub fn decode_event(
    contract_id: &str,
    raw_topics: &[String],
    raw_value: &str,
    spec_cache: &SpecCache,
) -> Option<DecodedEvent> {
    let spec = spec_cache.get(contract_id)?.as_ref()?;
    let topics = raw_topics
        .iter()
        .filter_map(|t| ScVal::from_xdr_base64(t, Limits::none()).ok())
        .collect::<Vec<_>>();
    if topics.len() != raw_topics.len() {
        return None;
    }
    let data = ScVal::from_xdr_base64(raw_value, Limits::none()).ok()?;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        spec.decode_event(contract_id, &topics, &data)
            .inspect_err(|e| tracing::debug!("Failed to decode event for {contract_id}: {e}"))
            .ok()
    }))
    .unwrap_or_else(|_| {
        tracing::debug!("decode_event panicked for {contract_id}");
        None
    })
}
