#![no_std]
use soroban_sdk::{contractimpl, vec, Address, Env, String, Symbol, Vec};

pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn hello(env: Env, world: Symbol) -> Vec<Symbol> {
        vec![&env, Symbol::short("Hello"), world]
    }

    pub fn world(env: Env, hello: Symbol) -> Vec<Symbol> {
        vec![&env, Symbol::short("Hello"), hello]
    }

    pub fn not(env: Env, boolean: bool) -> Vec<bool> {
        vec![&env, !boolean]
    }

    pub fn auth(env: Env, addr: Address, world: Symbol) -> Vec<Symbol> {
        addr.require_auth();
        // Emit test event
        env.events().publish(("auth",), world.clone());
        vec![&env, Symbol::short("Hello"), world]
    }

    #[allow(unused_variables)]
    pub fn multi_word_cmd(env: Env, contract_owner: String) {}
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
