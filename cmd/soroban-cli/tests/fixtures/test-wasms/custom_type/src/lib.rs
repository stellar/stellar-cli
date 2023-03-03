#![no_std]
use soroban_sdk::{
    contractimpl, contracttype, symbol, vec, Address, Bytes, Env, Map, Set, Symbol, Vec,
};

pub struct Contract;

/// Test Struct Docs
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
    pub fn hello(_env: Env, hello: Symbol) -> Symbol {
        hello
    }

    pub fn u32_(_env: Env, u32_: u32) -> u32 {
        u32_
    }

    pub fn i32_(_env: Env, i32_: i32) -> i32 {
        i32_
    }

    pub fn i64_(_env: Env, i64_: i64) -> i64 {
        i64_
    }

    pub fn strukt_hel(env: Env, strukt: Test) -> Vec<Symbol> {
        vec![&env, symbol!("Hello"), strukt.c]
    }

    /// Example contract method passing in a struct
    pub fn strukt(_env: Env, strukt: Test) -> Test {
        strukt
    }

    pub fn simple(_env: Env, simple: SimpleEnum) -> SimpleEnum {
        simple
    }

    pub fn complex(_env: Env, complex: ComplexEnum) -> ComplexEnum {
        complex
    }

    pub fn address(_env: Env, address: Address) -> Address {
        address
    }

    pub fn bytes(_env: Env, bytes: Bytes) -> Bytes {
        bytes
    }

    pub fn card(_env: Env, card: RoyalCard) -> RoyalCard {
        card
    }

    pub fn boolean(_: Env, boolean: bool) -> bool {
        boolean
    }

    pub fn not(_env: Env, boolean: bool) -> bool {
        !boolean
    }

    pub fn i128(_env: Env, i128: i128) -> i128 {
        i128
    }

    pub fn u128(_env: Env, u128: u128) -> u128 {
        u128
    }

    pub fn multi_args(_env: Env, a: u32, b: bool) -> u32 {
        if b {
            a
        } else {
            0
        }
    }

    pub fn map(_env: Env, map: Map<u32, bool>) -> Map<u32, bool> {
        map
    }

    pub fn set(_env: Env, set: Set<u32>) -> Set<u32> {
        set
    }

    pub fn vec(_env: Env, vec: Vec<u32>) -> Vec<u32> {
        vec
    }

    pub fn tuple(_env: Env, tuple: (Symbol, u32)) -> (Symbol, u32) {
        tuple
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

        let res = client.strukt_hel(&Test {
            a: 0,
            b: false,
            c: symbol!("world"),
        });
        assert_eq!(res, vec![&env, symbol!("Hello"), symbol!("world")]);
    }
}
