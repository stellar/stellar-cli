#![no_std]
use soroban_sdk::{contractimpl, symbol, vec, Env, Symbol, Vec};

pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn hello(env: Env, s: Symbol) -> Vec<Symbol> {
        vec![&env, symbol!("Hello"), s]
    }
}

#[cfg(test)]
mod test {
    use soroban_sdk::{symbol, vec, Env};

    use crate::{Contract, ContractClient};

    #[test]
    fn test_hello() {
        let env = Env::default();
        let contract_id = env.register_contract(None, Contract);
        let client = ContractClient::new(&env, &contract_id);

        let res = client.hello(&symbol!("world"));
        assert_eq!(res, vec![&env, symbol!("Hello"), symbol!("world")]);
    }
}
