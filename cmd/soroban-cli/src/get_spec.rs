use soroban_env_host::xdr;

use soroban_env_host::xdr::{
    ContractDataEntry, ContractExecutable, ScSpecEntry, ScVal, ScContractInstance,
};

use soroban_spec::read::FromWasmError;
pub use soroban_spec_tools::contract as contract_spec;

use crate::rpc;
use crate::commands::{config::data, global};


#[derive(thiserror::Error, Debug)]
pub enum GetSpecError {
    #[error("parsing contract spec: {0}")]
    CannotParseContractSpec(FromWasmError),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("missing result")]
    MissingResult,
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

///
/// # Errors
pub async fn get_remote_contract_spec(
    contract_id: &[u8; 32],
    rpc_url: &str,
    global_args: Option<&global::Args>,
) -> Result<Vec<ScSpecEntry>, GetSpecError> {
    let client = rpc::Client::new(rpc_url)?;
    // Get contract data
    let r = client.get_contract_data(&contract_id).await?;
    tracing::trace!("{r:?}");
    
    let ContractDataEntry {
        val: ScVal::ContractInstance(ScContractInstance { executable, .. }),
        ..
    } = r
    else {
        return Err(GetSpecError::MissingResult);
    };

    // Get the contract spec entries based on the executable type
    let spec_entries = match executable {
        ContractExecutable::Wasm(hash) => {
            let hash = hash.to_string();
            if let Ok(entries) = data::read_spec(&hash) {
                entries
            } else {
                let res = client.get_remote_contract_spec(&contract_id).await?;
                if global_args.map_or(true, |a| !a.no_cache) {
                    data::write_spec(&hash, &res)?;
                }
                res
            }
        }
        ContractExecutable::StellarAsset => {
            soroban_spec::read::parse_raw(&soroban_sdk::token::StellarAssetSpec::spec_xdr())?
        }
    };

    Ok(spec_entries)
}

