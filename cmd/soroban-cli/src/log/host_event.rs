use soroban_env_host::events::HostEvent;

pub fn host_events(events: &[HostEvent]) {
    for (i, event) in events.iter().enumerate() {
        tracing::info!("{i}: {event:#?}");
    }
}
