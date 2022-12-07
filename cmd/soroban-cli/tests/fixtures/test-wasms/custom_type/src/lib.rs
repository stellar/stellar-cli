#![no_std]
use soroban_sdk::{contractimpl, contracttype, symbol, vec, Env, Symbol, Vec};

pub struct Contract;

#[contracttype]
pub struct Test {
    pub a: u32,
    pub b: bool,
    pub c: Symbol,
}

#[contractimpl]
impl Contract {
    pub fn hello(env: Env, test: Test) -> Vec<Symbol> {
        vec![&env, symbol!("Hello"), test.c]
    }
}

#[cfg(test)]
mod test {
    use soroban_sdk::{symbol, vec, Env};

    use crate::{Contract, ContractClient, Test};

    #[test]
    fn test_hello() {
        let env = Env::default();
        let contract_id = env.register_contract(None, Contract);
        let client = ContractClient::new(&env, &contract_id);

        let res = client.hello(&Test {
            a: 0,
            b: false,
            c: symbol!("world"),
        });
        assert_eq!(res, vec![&env, symbol!("Hello"), symbol!("world")]);
    }
}
