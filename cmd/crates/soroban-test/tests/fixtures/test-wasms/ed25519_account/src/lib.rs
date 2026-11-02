#![no_std]
//! A smart account with one ed25519 signer, for testing signer plugins.
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractimpl,
    crypto::Hash,
    symbol_short, BytesN, Env, Symbol, Vec,
};

const SIGNER: Symbol = symbol_short!("signer");

#[contract]
pub struct Contract;

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
    NoSigner = 1,
}

#[contractimpl]
impl Contract {
    pub fn __constructor(env: Env, signer: BytesN<32>) {
        env.storage().instance().set(&SIGNER, &signer);
    }
}

#[contractimpl]
impl CustomAccountInterface for Contract {
    type Error = Error;
    type Signature = BytesN<64>;

    #[allow(non_snake_case)]
    fn __check_auth(
        env: Env,
        signature_payload: Hash<32>,
        signature: BytesN<64>,
        _auth_contexts: Vec<Context>,
    ) -> Result<(), Error> {
        let signer: BytesN<32> = env
            .storage()
            .instance()
            .get(&SIGNER)
            .ok_or(Error::NoSigner)?;
        env.crypto()
            .ed25519_verify(&signer, &signature_payload.into(), &signature);
        Ok(())
    }
}
