use std::str::FromStr;

use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::tx::fetch,
    config::{self, data, network},
    resources,
    signer::{self, Signer},
    xdr::{
        self, FeeBumpTransaction, FeeBumpTransactionExt, FeeBumpTransactionInnerTx, Transaction,
        TransactionEnvelope,
    },
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
    // cache user set inclusion fee
    let inclusion_fee = tx.fee;
    let assembled_resp = simulate_and_assemble_transaction(
        client,
        tx,
        resources.resource_config(),
        resources.resource_fee,
    )
    .await?;
    let assembled = resources.apply_to_assembled_txn(assembled_resp);
    let mut txn = Box::new(assembled.transaction().clone());
    let sim_res = assembled.sim_response();

    let rpc_uri = Url::from_str(client.base_url())
        .map_err(|_| config::network::Error::InvalidUrl(client.base_url().to_string()))?;
    if !no_cache {
        data::write(sim_res.clone().into(), &rpc_uri)?;
    }

    // Need to sign all auth entries
    if let Some(mut tx) = config
        .sign_soroban_authorizations(&txn, auth_signers)
        .await?
    {
        // if we added signatures to auth entries, we need to re-simulate to correctly account
        // for resource usage when validating the auth entry signatures.
        tx.fee = inclusion_fee; // reset inclusion fee to ensure assembled fee is correct
        let new_assembled_resp = simulate_and_assemble_transaction(
            client,
            &tx,
            resources.resource_config(),
            resources.resource_fee,
        )
        .await?;
        let new_assembled = resources.apply_to_assembled_txn(new_assembled_resp);
        *txn = new_assembled.transaction().clone();
    }

    let mut signed_tx = config.sign(*txn, quiet).await?;

    // If the simulation detected the need for a fee bump,
    // wrap the transaction in a fee bump with the appropriate fee amount
    if let Some(fee_bump_fee) = assembled.fee_bump_fee() {
        let fee_bump_inner = match signed_tx {
            TransactionEnvelope::Tx(tx_env) => FeeBumpTransactionInnerTx::Tx(tx_env),
            _ => {
                return Err(config::Error::Signer(
                    signer::Error::UnsupportedTransactionEnvelopeType,
                )
                .into())
            }
        };
        let fee_bump = FeeBumpTransaction {
            fee_source: tx.source_account.clone(),
            fee: fee_bump_fee,
            inner_tx: fee_bump_inner,
            ext: FeeBumpTransactionExt::V0,
        };
        signed_tx = config.sign_fee_bump(fee_bump, quiet).await?;
    }

    let res = client.send_transaction_polling(&signed_tx).await?;

    resources.print_cost_info(&res)?;

    if !no_cache {
        data::write(res.clone().try_into()?, &rpc_uri)?;
    }

    Ok(res)
}
