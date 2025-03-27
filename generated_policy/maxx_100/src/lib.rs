#![no_std]

use smart_wallet_interface::{
    types::{Signer, SignerKey, SignerLimits, SignerStorage, SignerExpiration},
    PolicyInterface, SmartWalletClient,
};
use soroban_sdk::{
    auth::{Context, ContractContext},
    contract, contracterror, contractimpl, contracttype, map, panic_with_error, symbol_short,
    Address, BytesN, Env, TryFromVal, Vec,
};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum StorageKey {
    Admin,
    Signer(BytesN<32>),
}

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotAllowed = 1,
    InvalidSigner = 2,
    InvalidAmount = 3,
    InvalidContext = 4,
    NotInitialized = 5,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&StorageKey::Admin, &admin);
    }

    pub fn add_authorized_signer(env: Env, signer: BytesN<32>) {
        let admin = env.storage().instance().get::<StorageKey, Address>(&StorageKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
            
        admin.require_auth();

        // Add signer to smart wallet with policy restrictions
        SmartWalletClient::new(&env, &admin).add_signer(&Signer::Ed25519(
            signer.clone(),
            SignerExpiration(Some(u32::MAX)),
            SignerLimits(Some(map![
                &env,
                (
                    env.current_contract_address(),
                    Some(vec![
                        &env,
                        SignerKey::Policy(env.current_contract_address())
                    ])
                )
            ])),
            SignerStorage::Persistent,
        ));

        // Store signer in our policy contract
        env.storage().persistent().set(&StorageKey::Signer(signer), &true);
    }

    pub fn remove_authorized_signer(env: Env, signer: BytesN<32>) {
        let admin = env.storage().instance().get::<StorageKey, Address>(&StorageKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));
            
        admin.require_auth();

        SmartWalletClient::new(&env, &admin).remove_signer(&SignerKey::Ed25519(signer.clone()));
        env.storage().persistent().remove(&StorageKey::Signer(signer));
    }
}

#[contractimpl]
impl PolicyInterface for Contract {
    fn policy__(env: Env, source: Address, signer: SignerKey, contexts: Vec<Context>) {
        // First validate the signer
        let authorized = match signer {
            SignerKey::Ed25519(key) => {
                // Check if signer is in our authorized list
                env.storage()
                    .persistent()
                    .get::<StorageKey, bool>(&StorageKey::Signer(key))
                    .unwrap_or(false)
            },
            _ => false
        };

        if !authorized {
            panic_with_error!(&env, Error::InvalidSigner);
        }

        for context in contexts.iter() {
            match context {
                Context::Contract(ContractContext { fn_name, args, .. }) => {
                    // Check if function is explicitly allowed in rules
                    if fn_name == Symbol::new(&env, "add_contact") {
                        return; // Function is allowed without restrictions
                    }
                    if fn_name == Symbol::new(&env, "edit_contact") {
                        return; // Function is allowed without restrictions
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
                    if fn_name == Symbol::new(&env, "transfer") {
                        // Function is allowed, apply its specific restrictions if any
                        if let Some(amount_val) = args.get(2) {  // transfer function has amount at index 2
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