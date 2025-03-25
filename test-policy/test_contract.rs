#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct TestContract;

#[contractimpl]
impl TestContract {
    pub fn transfer(env: Env, to: Address, amount: i128) -> i128 {
        amount
    }
} 