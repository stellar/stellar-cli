#![no_std]
use soroban_sdk::{contractimpl, contracttype, symbol, vec, Env, Symbol, Vec};

pub struct Contract;

#[contracttype]
pub struct Test {
    pub a: u32,
    pub b: bool,
    pub c: Symbol,
}

#[contracttype]
pub enum SimpleEnum {
    First,
    Second,
    Third,
}

#[contracttype]
pub struct TupleStruct(Test, SimpleEnum);

#[contracttype]
pub enum ComplexEnum {
    Struct(Test),
    Tuple(TupleStruct),
    Enum(SimpleEnum),
}

#[contractimpl]
impl Contract {
    pub fn hello(env: Env, test: Test) -> Vec<Symbol> {
        vec![&env, symbol!("Hello"), test.c]
    }

    pub fn enum_2_str(env: Env, simple: SimpleEnum) -> Vec<SimpleEnum> {
        vec![&env, simple]
    }

    pub fn e_2_s(env: Env, complex: ComplexEnum) -> Vec<ComplexEnum> {
        vec![&env, complex]
    }
}

#[cfg(test)]
mod test {
    use soroban_sdk::{log, symbol, vec, Env};

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
