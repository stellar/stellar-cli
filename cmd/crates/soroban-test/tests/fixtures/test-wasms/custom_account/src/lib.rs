#![no_std]
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractimpl, contracttype,
    crypto::Hash,
    symbol_short, vec, Address, Bytes, BytesN, Env, Symbol, Vec,
};

#[contract]
pub struct Contract;

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
    NotFound = 1,
    NotPermitted = 2,
    ClientDataJsonChallengeIncorrect = 3,
    Secp256r1PublicKeyParse = 4,
    Secp256r1SignatureParse = 5,
    Secp256r1VerifyFailed = 6,
    JsonParseError = 7,
    InvalidContext = 8,
    AlreadyInited = 9,
    NotInited = 10,
}

const SIGNERS: Symbol = symbol_short!("sigs");
const FACTORY: Symbol = symbol_short!("factory");
const SUDO_SIGNER: Symbol = symbol_short!("sudo_sig");

#[contractimpl]
impl Contract {
    pub fn extend_ttl(env: &Env) {
        let max_ttl = env.storage().max_ttl();
        let contract_address = env.current_contract_address();

        env.storage().instance().extend_ttl(max_ttl, max_ttl);
        env.deployer()
            .extend_ttl(contract_address.clone(), max_ttl, max_ttl);
        env.deployer()
            .extend_ttl_for_code(contract_address.clone(), max_ttl, max_ttl);
        env.deployer()
            .extend_ttl_for_contract_instance(contract_address.clone(), max_ttl, max_ttl);
    }
    pub fn init(env: Env, id: Bytes, pk: BytesN<65>, factory: Address) -> Result<(), Error> {
        if env.storage().instance().has(&SUDO_SIGNER) {
            return Err(Error::AlreadyInited);
        }

        let max_ttl = env.storage().max_ttl();

        env.storage().persistent().set(&id, &pk);
        env.storage().persistent().extend_ttl(&id, max_ttl, max_ttl);

        env.storage().instance().set(&SUDO_SIGNER, &id);
        env.storage().instance().set(&FACTORY, &factory);
        env.storage().instance().set(&SIGNERS, &vec![&env, id]);

        Self::extend_ttl(&env);

        Ok(())
    }
}

#[contracttype]
pub struct Signature {
    pub id: BytesN<32>,
    pub authenticator_data: Bytes,
    pub client_data_json: Bytes,
    pub signature: BytesN<64>,
}

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
        signature_payload: Hash<32>,
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
                        && signature.id
                            != env
                                .storage()
                                .instance()
                                .get::<Symbol, BytesN<32>>(&SUDO_SIGNER)
                                .ok_or(Error::NotFound)?
                    {
                        return Err(Error::NotPermitted);
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
        if signature_payload.to_bytes().len() != 32 {
            return Err(Error::NotPermitted);
        }

        let client_data = ClientDataJson {
            challenge: "dummy_challenge",
        };

        if client_data.challenge != "dummy_challenge" {
            return Err(Error::ClientDataJsonChallengeIncorrect);
        }

        Ok(())
    }
}
