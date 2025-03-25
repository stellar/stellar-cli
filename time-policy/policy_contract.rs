use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct TimeBasedPolicy;

#[contractimpl]
impl TimeBasedPolicy {
    pub fn check_policy(env: Env, target: Address) -> bool {
        let created_at = env.storage().instance().get::<_, u64>(&target).unwrap_or(0);
        if created_at == 0 {
            env.storage().instance().set(&target, &env.ledger().timestamp());
            return true;
        }
        
        let elapsed = env.ledger().timestamp() - created_at;
        elapsed <= 3600
    }
}
