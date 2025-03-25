use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct AmountBasedPolicy;

#[contractimpl]
impl AmountBasedPolicy {
    pub fn check_policy(env: Env, target: Address, amount: u64) -> bool {
        let used = env.storage().instance().get::<_, u64>(&target).unwrap_or(0);
        let new_total = used.saturating_add(amount);
        
        if new_total <= 5000 {
            env.storage().instance().set(&target, &new_total);
            true
        } else {
            false
        }
    }
}
