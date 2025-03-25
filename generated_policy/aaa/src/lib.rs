#![no_std]
use soroban_sdk::{contract, contractimpl, Env};
use smart_wallet_interface::PolicyTrait;

#[contract]
pub struct aaa;

#[contractimpl]
impl PolicyTrait for aaa {
    #![no_std]
use soroban_sdk::{
    auth::{Context, ContractContext},
    contract, contracterror, contractimpl, panic_with_error, symbol_short,
    Address, Env, Vec,
};
use smart_wallet_interface::{types::SignerKey, PolicyInterface};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotAllowed &#x3D; 1,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl PolicyInterface for Contract {
    fn policy__(env: Env, _source: Address, _signer: SignerKey, contexts: Vec&lt;Context&gt;) {
        for context in contexts.iter() {
            if let Context::Contract(ContractContext { fn_name, .. }) &#x3D; context {
                if fn_name &#x3D;&#x3D; symbol_short!(&quot;submit&quot;) { return; }
            }
        }
        panic_with_error!(&amp;env, Error::NotAllowed)
    }
}
}