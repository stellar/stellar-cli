#![no_std]
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractimpl, contracttype,
    crypto::Hash,
    symbol_short, Bytes, BytesN, Env, Symbol, Vec,
};

#[contract]
pub struct Contract;

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
    NotFound = 1,
    NotPermitted = 2,
    ClientDataJsonChallengeIncorrect = 3,
    JsonParseError = 7,
    InvalidContext = 8,
}

const SUDO_SIGNER: Symbol = symbol_short!("sudo_sig");

#[contracttype]
pub struct Signature {
    pub id: BytesN<32>,
    pub authenticator_data: Bytes,
    pub client_data_json: Bytes,
    pub signature: BytesN<64>,
}

// Dummy implementation for the demo
#[derive(Debug)]
struct ClientDataJson<'a> {
    challenge: &'a str,
}

#[contractimpl]
impl CustomAccountInterface for Contract {
    type Error = Error;
    type Signature = Signature;

    #[allow(non_snake_case)]
    fn __check_auth(
        env: Env,
        _signature_payload: Hash<32>,
        signature: Signature,
        auth_contexts: Vec<Context>,
    ) -> Result<(), Error> {
        // Only the sudo signer can `add_sig`, `rm_sig` and `resudo`
        for context in auth_contexts.iter() {
            match context {
                Context::Contract(c) => {
                    if c.contract == env.current_contract_address()
                        && (c.fn_name == Symbol::new(&env, "add_sig")
                            || c.fn_name == Symbol::new(&env, "rm_sig")
                            || c.fn_name == Symbol::new(&env, "resudo"))
                    {
                        if signature.id
                            != env
                                .storage()
                                .instance()
                                .get::<Symbol, BytesN<32>>(&SUDO_SIGNER)
                                .ok_or(Error::NotFound)?
                        {
                            return Err(Error::NotPermitted);
                        }
                    }
                }
                Context::CreateContractHostFn(_) => return Err(Error::InvalidContext),
            };
        }

        // Dummy public key verification check
        env.storage()
            .persistent()
            .get::<BytesN<32>, Bytes>(&signature.id)
            .ok_or(Error::NotFound)?;

        let client_data = ClientDataJson {
            challenge: "dummy_challenge",
        };

        if client_data.challenge != "dummy_challenge" {
            return Err(Error::ClientDataJsonChallengeIncorrect);
        }

        Ok(())
    }
}
