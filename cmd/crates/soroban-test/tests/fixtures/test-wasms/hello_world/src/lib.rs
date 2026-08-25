#![no_std]
use soroban_sdk::{
    contract, contractevent, contractimpl, log, symbol_short, vec, Address, BytesN,
    ContractExecutable, ContractExecutableRef, Env, String, Symbol, Vec,
};

const COUNTER: Symbol = symbol_short!("COUNTER");

#[contractevent]
pub struct AuthEvent {
    pub world: Symbol,
}

#[contract]
pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn hello(env: Env, world: Symbol) -> Vec<Symbol> {
        vec![&env, symbol_short!("Hello"), world]
    }

    pub fn world(env: Env, hello: Symbol) -> Vec<Symbol> {
        vec![&env, symbol_short!("Hello"), hello]
    }

    pub fn not(env: Env, boolean: bool) -> Vec<bool> {
        vec![&env, !boolean]
    }

    pub fn auth(env: Env, addr: Address, world: Symbol) -> Address {
        addr.require_auth();
        // Emit test event
        AuthEvent { world }.publish(&env);

        addr
    }

    // get current count
    pub fn get_count(env: Env) -> u32 {
        env.storage().persistent().get(&COUNTER).unwrap_or(0)
    }

    // increment count and return new one
    pub fn inc(env: Env) -> u32 {
        let mut count: u32 = env.storage().persistent().get(&COUNTER).unwrap_or(0); // Panic if the value of COUNTER is not u32.
        log!(&env, "count: {}", count);

        // Increment the count.
        count += 1;

        // Save the count.
        env.storage().persistent().set(&COUNTER, &count);
        count
    }

    pub fn prng_u64_in_range(env: Env, low: u64, high: u64) -> u64 {
        env.prng().gen_range(low..=high)
    }

    pub fn upgrade_contract(env: Env, hash: BytesN<32>) {
        env.deployer()
            .update_current_contract(ContractExecutable::Wasm(hash));
    }

    // --- CAP-85: externally managed executables (beacon-proxy pattern) ---

    // Publish (create or update) an executable reference entry owned by this
    // contract, keyed by `tag`, pointing at an already-uploaded `wasm_hash`.
    pub fn publish(env: Env, tag: String, wasm_hash: BytesN<32>) {
        env.executable_refs().set(&tag, &wasm_hash);
    }

    // Read the Wasm hash the executable reference entry `tag` points at.
    pub fn get_ref(env: Env, tag: String) -> Option<BytesN<32>> {
        env.executable_refs().get(&tag)
    }

    // Deploy a fresh contract whose executable is the reference entry `tag`
    // owned by this contract.
    pub fn deploy_ref(env: Env, tag: String) -> Address {
        let salt = BytesN::from_array(&env, &[0u8; 32]);
        env.deployer().with_current_contract(salt).deploy_contract(
            ContractExecutable::ExternalRef(ContractExecutableRef {
                owner: env.current_contract_address(),
                tag,
            }),
            (),
        )
    }

    #[allow(unused_variables)]
    pub fn multi_word_cmd(env: Env, contract_owner: String) {}

    /// Logs a string with `hello ` in front.
    pub fn log(env: Env, str: Symbol) {
        // Emit the event format expected by the test using the deprecated API
        #[allow(deprecated)]
        env.events()
            .publish((symbol_short!("hello"), Symbol::new(&env, "")), str.clone());
        log!(&env, "hello {}", str);
    }
}

#[cfg(test)]
mod test {
    use soroban_sdk::{symbol_short, vec, Env};

    use crate::{Contract, ContractClient};

    #[test]
    fn test_hello() {
        let env = Env::default();
        let contract_id = env.register(Contract, ());
        let client = ContractClient::new(&env, &contract_id);
        let world = symbol_short!("world");
        let res = client.hello(&world);
        assert_eq!(res, vec![&env, symbol_short!("Hello"), world]);
    }
}
