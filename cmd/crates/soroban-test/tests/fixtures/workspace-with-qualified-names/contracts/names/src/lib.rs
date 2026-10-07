#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, Env};

pub mod a {
    use soroban_sdk::contracttype;

    #[contracttype]
    pub struct State {
        pub count: u32,
    }
}

pub mod b {
    use soroban_sdk::contracttype;

    #[contracttype]
    pub struct State {
        pub enabled: bool,
    }

    #[contracttype]
    pub struct Wrapper {
        pub state: State,
    }
}

#[contractevent]
pub struct Updated {
    pub count: u32,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn get_a(_env: Env, state: a::State) -> a::State {
        state
    }

    pub fn get_b(_env: Env, wrapper: b::Wrapper) -> b::State {
        wrapper.state
    }

    pub fn update(env: Env, count: u32) {
        Updated { count }.publish(&env);
    }
}
