#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, Address, Env, Symbol,
};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotAllowed = 1,
}

#[contract]
pub struct FunctionPolicy;

#[contractimpl]
impl FunctionPolicy {
    pub fn check_policy(env: Env, function_name: Symbol) -> bool {
        function_name == Symbol::new(&env, "do_math")
    }

    pub fn get_allowed_function(env: Env) -> Symbol {
        Symbol::new(&env, "do_math")
    }
}
