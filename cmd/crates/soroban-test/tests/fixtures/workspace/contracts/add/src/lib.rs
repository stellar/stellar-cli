#![no_std]
use soroban_sdk::{contract, contractimpl, contractmeta};

#[contract]
pub struct Contract;

contractmeta!(key = "Description", val = "A test add contract");

#[contractimpl]
impl Contract {
    pub fn add(x: u64, y: u64) -> u128 {
        let x: u128 = x.into();
        let y: u128 = y.into();
        x + y
    }
}
