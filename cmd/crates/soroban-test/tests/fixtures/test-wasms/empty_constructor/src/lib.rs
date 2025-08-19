#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

#[contract]
pub struct Contract;
const COUNTER: Symbol = symbol_short!("COUNTER");

#[contractimpl]
impl Contract {
    /// Example constructor
    pub fn __constructor(env: Env) {
        env.storage().persistent().set(&COUNTER, &0);
    }
    /// Counter value
    pub fn counter(env: Env) -> u32 {
        env.storage().persistent().get(&COUNTER).unwrap()
    }
}
