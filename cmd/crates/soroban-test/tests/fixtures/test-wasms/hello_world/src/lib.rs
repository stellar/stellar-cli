#![no_std]
use soroban_sdk::{contract, log, symbol_short, vec, Address, Env, String, Symbol, Vec};

#[contract]
pub struct Contract;

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

    use soroban_sdk::{vec, Env, Symbol};

    use crate::{Contract, ContractClient};

    #[test]
    fn test_hello() {
        let env = Env::default();
        let contract_id = env.register_contract(None, Contract);
        let client = ContractClient::new(&env, &contract_id);
        let world = Symbol::short("world");
        let res = client.hello(&world);
        assert_eq!(res, vec![&env, Symbol::short("Hello"), world]);
    }
}
