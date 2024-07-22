#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, IntoVal};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn call(env: &Env, a: Address, x: u64, y: u64) -> u128 {
        env.invoke_contract(&a, &symbol_short!("add"), (x, y).into_val(env))
    }
}
