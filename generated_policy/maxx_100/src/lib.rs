#![no_std]

use smart_wallet_interface::{types::SignerKey, PolicyInterface};
use soroban_sdk::{
    auth::{Context, ContractContext},
    contract, contracterror, contractimpl, panic_with_error, Symbol,
    Address, Env, TryFromVal, Vec, BytesN,
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
        // First verify the signer
        if signer != SignerKey::Ed25519(BytesN::from_array(&env, &[
            // Your signer's public key bytes here
            0x46, 0x61, 0x20, 0xb2, 0x2a, 0x88, 0x9f, 0x89, 0x6d, 0x1f, 0x71, 0x28,
            0xc3, 0x9a, 0x32, 0x48, 0x2c, 0x90, 0x44, 0x52, 0x82, 0x86, 0xef, 0xe3,
            0x0e, 0x35, 0xce, 0x37, 0x3c, 0xc9, 0xc8, 0x66
        ])) {
            panic_with_error!(&env, Error::InvalidSigner);
        }

        for context in contexts.iter() {
            match context {
                Context::Contract(ContractContext { fn_name, args, .. }) => {
                    // Check if function is explicitly allowed in rules
                    if fn_name == Symbol::new(&env, "add_contact") {
                    }
                    if fn_name == Symbol::new(&env, "edit_contact") {
                    }
                    if fn_name == Symbol::new(&env, "transfer_to_contact") {
                        // Function is allowed, apply its specific restrictions if any
                        if let Some(amount_val) = args.get(3) {
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