use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

#[contract]
pub struct MultiSigPolicy;

#[contractimpl]
impl MultiSigPolicy {
    pub fn check_policy(env: Env, signatures: Vec<Address>) -> bool {
        signatures.len() >= 3
    }
}
