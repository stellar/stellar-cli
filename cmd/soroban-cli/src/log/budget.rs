use soroban_env_host::budget::Budget;

pub fn budget(budget: &Budget) {
    tracing::debug!(?budget);
}
