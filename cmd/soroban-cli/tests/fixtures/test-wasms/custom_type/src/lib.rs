#![no_std]
use soroban_sdk::{contractimpl, contracttype, symbol, vec, AccountId, Bytes, Env, Symbol, Vec};

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
#[derive(Clone, Copy)]
// The `repr` attribute is here to specify the memory alignment for this type
#[repr(u32)]
pub enum RoyalCard {
    // TODO: create the fields here for your `RoyalCard` type
    Jack = 11,  // delete this
    Queen = 12, // delete this
    King = 13,  // delete this
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
    pub fn hello(env: Env, world: Symbol) -> Vec<Symbol> {
        vec![&env, symbol!("Hello"), world]
    }

    pub fn u32_(_env: Env, u32_: u32) -> u32 {
        u32_
    }
    pub fn strukt(env: Env, strukt: Test) -> Vec<Symbol> {
        vec![&env, symbol!("Hello"), strukt.c]
    }

    pub fn enum_2_str(env: Env, simple: SimpleEnum) -> Vec<SimpleEnum> {
        vec![&env, simple]
    }

    pub fn e_2_s(env: Env, complex: ComplexEnum) -> Vec<ComplexEnum> {
        vec![&env, complex]
    }

    pub fn account(env: Env, account_id: AccountId) -> Vec<AccountId> {
        vec![&env, account_id]
    }

    pub fn bytes(env: Env, bytes: Bytes) -> Vec<Bytes> {
        vec![&env, bytes]
    }

    pub fn card(env: Env, card: RoyalCard) -> Vec<RoyalCard> {
        vec![&env, card]
    }

    pub fn not(_env: Env, boolean: bool) -> bool {
        !boolean
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

        let res = client.strukt(&Test {
            a: 0,
            b: false,
            c: symbol!("world"),
        });
        assert_eq!(res, vec![&env, symbol!("Hello"), symbol!("world")]);
    }
}
