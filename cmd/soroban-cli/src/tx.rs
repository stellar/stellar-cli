use std::str::FromStr;

use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::tx::fetch,
    config::{self, data, network},
    resources,
    signer::Signer,
    xdr::{self, Transaction},
};
use soroban_rpc::GetTransactionResponse;
use url::Url;

pub mod builder;

/// 10,000,000 stroops in 1 XLM
pub const ONE_XLM: i64 = 10_000_000;

/// Simulates, signs, and sends a transaction to the network.
///
/// Returns the `GetTransactionResponse` from the network.
pub async fn sim_sign_and_send_tx<E>(
    client: &soroban_rpc::Client,
    tx: &Transaction,
    config: &config::Args,
    resources: &resources::Args,
    auth_signers: &[Signer],
    quiet: bool,
    no_cache: bool,
) -> Result<GetTransactionResponse, E>
where
    E: From<soroban_rpc::Error>
        + From<config::Error>
        + From<fetch::Error>
        + From<data::Error>
        + From<network::Error>
        + From<xdr::Error>,
{
    let txn = simulate_and_assemble_transaction(
        client,
        tx,
        resources.resource_config(),
        resources.resource_fee,
    )
    .await?;
    let assembled = resources.apply_to_assembled_txn(txn);
    let mut txn = Box::new(assembled.transaction().clone());
    let sim_res = assembled.sim_response();

    let rpc_uri = Url::from_str(client.base_url())
        .map_err(|_| config::network::Error::InvalidUrl(client.base_url().to_string()))?;
    if !no_cache {
        data::write(sim_res.clone().into(), &rpc_uri)?;
    }

    // Need to sign all auth entries
    if let Some(tx) = config
        .sign_soroban_authorizations(&txn, auth_signers)
        .await?
    {
        *txn = tx;
    }

    let res = client
        .send_transaction_polling(&config.sign(*txn, quiet).await?)
        .await?;

    resources.print_cost_info(&res)?;

    if !no_cache {
        data::write(res.clone().try_into()?, &rpc_uri)?;
    }

    Ok(res)
}
