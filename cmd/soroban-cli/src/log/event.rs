use soroban_env_host::events::HostEvent;

pub fn events(events: &[HostEvent]) {
    for (num, event) in events.iter().enumerate() {
        if let soroban_env_host::events::Event::Debug(log) = &event.event {
            contract_log::contract_log(log);
        } else {
            tracing::debug!(num, event = ?event.event);
        }
    }
}

mod contract_log {
    use soroban_env_host::events::DebugEvent;

    pub fn contract_log(log: &DebugEvent) {
        tracing::info!(log = log.to_string());
    }
}
