#![no_std]
use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn add(x: u64, y: u64) -> u128 {
        let x: u128 = x.into();
        let y: u128 = y.into();
        x + y
    }
}
