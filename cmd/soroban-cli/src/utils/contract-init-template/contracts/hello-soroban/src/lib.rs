#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, vec, Env, Symbol, Vec};

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    /// Say Hello to someone or something.
    /// Returns a length-2 vector/array containing 'Hello' and then the value passed as `to`.
    pub fn hello(env: Env, to: Symbol) -> Vec<Symbol> {
        vec![&env, symbol_short!("Hello"), to]
    }
}

#[cfg(test)]
mod test;
