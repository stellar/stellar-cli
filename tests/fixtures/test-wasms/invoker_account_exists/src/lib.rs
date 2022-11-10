#![no_std]
use soroban_sdk::{contracterror, contractimpl, panic_with_error, Address, Env};

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum Error {
    InvokerIsContract = 1,
}

pub struct Contract;

#[contractimpl]
impl Contract {
    pub fn invkexists(env: Env) -> bool {
        match env.invoker() {
            Address::Account(account_id) => env.accounts().get(&account_id).is_some(),
            Address::Contract(_) => panic_with_error!(&env, Error::InvokerIsContract),
        }
    }
}

#[cfg(test)]
mod test {
    use soroban_sdk::{testutils::Accounts, Env};

    use crate::{Contract, ContractClient};

    #[test]
    fn test_invoker() {
        let env = Env::default();
        let contract_id = env.register_contract(None, Contract);
        let client = ContractClient::new(&env, &contract_id);

        let addr = env.accounts().generate();
        let exists = client.with_source_account(&addr).invkexists();
        assert!(exists);
    }
}
