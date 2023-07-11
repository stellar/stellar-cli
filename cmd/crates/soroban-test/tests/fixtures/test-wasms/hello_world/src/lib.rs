#![no_std]
use soroban_sdk::{
    contract, contractimpl, log, symbol_short, vec, Address, Env, String, Symbol, Vec,
};

const COUNTER: Symbol = symbol_short!("COUNTER");

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn hello(env: Env, world: Symbol) -> Vec<Symbol> {
        vec![&env, symbol_short!("Hello"), world]
    }

    pub fn world(env: Env, hello: Symbol) -> Vec<Symbol> {
        vec![&env, symbol_short!("Hello"), hello]
    }

    pub fn not(env: Env, boolean: bool) -> Vec<bool> {
        vec![&env, !boolean]
    }

    pub fn auth(env: Env, addr: Address, world: Symbol) -> Address {
        addr.require_auth();
        // Emit test event
        env.events().publish(("auth",), world);

        addr
    }

    pub fn inc(env: Env) {
        let mut count: u32 = env.storage().temporary().get(&COUNTER).unwrap_or(0); // Panic if the value of COUNTER is not u32.
        log!(&env, "count: {}", count);

        // Increment the count.
        count += 1;

        // Save the count.
        env.storage().temporary().set(&COUNTER, &count);
    }

    #[allow(unused_variables)]
    pub fn multi_word_cmd(env: Env, contract_owner: String) {}
    /// Logs a string with `hello ` in front.
    pub fn log(env: Env, str: Symbol) {
        env.events().publish(
            (Symbol::new(&env, "hello"), Symbol::new(&env, "")),
            str.clone(),
        );
        log!(&env, "hello {}", str);
    }
}

#[cfg(test)]
mod test {
    use soroban_sdk::{symbol_short, vec, Env};

    use crate::{Contract, ContractClient};

    #[test]
    fn test_hello() {
        let env = Env::default();
        let contract_id = env.register_contract(None, Contract);
        let client = ContractClient::new(&env, &contract_id);
        let world = symbol_short!("world");
        let res = client.hello(&world);
        assert_eq!(res, vec![&env, symbol_short!("Hello"), world]);
    }
}
