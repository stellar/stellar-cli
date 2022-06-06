use stellar_contract_env_host::{xdr, ContractId};

pub const ZERO: ContractId = ContractId(xdr::Hash([0; 32]));
