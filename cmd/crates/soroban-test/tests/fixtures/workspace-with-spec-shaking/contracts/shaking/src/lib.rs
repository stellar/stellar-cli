#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, symbol_short, Env, Symbol};

#[contract]
pub struct Contract;

#[contracttype]
pub struct UsedStruct {
    pub a: u32,
    pub b: bool,
}

#[contracttype]
pub enum UsedEnum {
    First,
    Second,
}

#[contracttype]
pub struct UnusedStruct {
    pub x: u32,
    pub y: u32,
}

#[contracttype]
pub enum UnusedEnum {
    Alpha,
    Beta,
}

#[contractevent]
pub struct UsedEvent {
    pub value: u32,
}

#[contractevent]
pub struct UnusedEvent {
    pub data: Symbol,
}

#[contractimpl]
impl Contract {
    pub fn use_struct(_env: Env, strukt: UsedStruct) -> UsedStruct {
        strukt
    }

    pub fn use_enum(_env: Env, val: UsedEnum) -> UsedEnum {
        val
    }

    pub fn emit_event(env: Env) -> u32 {
        UsedEvent { value: 42 }.publish(&env);
        42
    }

    pub fn hello(env: Env) -> Symbol {
        symbol_short!("hello")
    }
}
