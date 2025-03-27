#![no_std]

use smart_wallet_interface::{types::SignerKey, PolicyInterface};
use soroban_sdk::{
    auth::{Context, ContractContext},
    contract, contracterror, contractimpl, panic_with_error, Symbol,
    Address, Env, TryFromVal, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotAllowed = 1,
    InvalidSigner = 2,
    InvalidAmount = 3,
    InvalidContext = 4,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl PolicyInterface for Contract {
    fn policy__(env: Env, source: Address, signer: SignerKey, contexts: Vec<Context>) {
        for context in contexts.iter() {
            match context {
                Context::Contract(ContractContext { fn_name, args, .. }) => {
                    // Check if function is explicitly allowed in rules
                    if fn_name == Symbol::new(&env, "add_contact") {
                        return; // Function is allowed without restrictions
                    }
                    if fn_name == Symbol::new(&env, "edit_contact") {
                    }
                    if fn_name == Symbol::new(&env, "transfer_to_contact") {
                        // Function is allowed, apply its specific restrictions if any
                        if let Some(amount_val) = args.get(2) {
                            if let Ok(amount) = i128::try_from_val(&env, &amount_val) {
                                if amount > 100 {
                                    panic_with_error!(&env, Error::InvalidAmount)
                                }
                            }
                        }

                        
                        return; // Function is allowed and passed all restrictions
                    }
                    
                    // If we get here, either the function wasn't in rules or wasn't enabled
                    panic_with_error!(&env, Error::NotAllowed);
                }
                _ => panic_with_error!(&env, Error::InvalidContext),
            }
        }

    }
}