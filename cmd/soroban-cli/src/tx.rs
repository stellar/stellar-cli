use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::tx::fetch,
    config::{self, data, network},
    print, resources,
    signer::{self, Signer},
    utils::transaction_env_hash,
    xdr::{
        self, FeeBumpTransaction, FeeBumpTransactionExt, FeeBumpTransactionInnerTx, Transaction,
        TransactionEnvelope,
    },
};
use soroban_rpc::GetTransactionResponse;

pub mod builder;

/// 10,000,000 stroops in 1 XLM
pub const ONE_XLM: i64 = 10_000_000;

/// Simulates, signs, and sends a transaction to the network.
///
/// This function handles a couple common tasks related to sending transactions:
/// * Log status messages to stderr when `quiet` is false
/// * Store results to the data cache when `no_cache` is false
/// * Logs a success message and block explorer link to stderr upon successful submission
///
/// Does not handle any logging related to the result, events, of effects of the transaction.
///
/// Returns the `GetTransactionResponse` from the network.
///
/// # Errors
/// If any step of the process fails (simulation, signing, sending)
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
    let print = print::Print::new(quiet);
    let network = config.get_network()?;
    print.infoln("Simulating transaction…");
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

    if !no_cache {
        data::write(sim_res.clone().into(), &network.rpc_uri()?)?;
    }

    // Need to sign all auth entries
    if let Some(tx) = config
        .sign_soroban_authorizations(&txn, auth_signers)
        .await?
    {
        *txn = tx;
    }

    let mut signed_tx = config.sign(*txn, quiet).await?;

    // If the simulation detected the need for a fee bump,
    // wrap the transaction in a fee bump with the appropriate fee amount
    if let Some(fee_bump_fee) = assembled.fee_bump_fee() {
        print.warnln(format!(
            "Wrapping transaction with a fee bump transaction due to a fee of {} XLM.",
            print::format_number(fee_bump_fee, 7)
        ));
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
    print.globeln("Sending transaction…");

    // returns an error if the transaction fails
    let res = client.send_transaction_polling(&signed_tx).await?;

    print.checkln("Transaction submitted successfully!");

    let tx_hash_bytes = transaction_env_hash(&signed_tx, &network.network_passphrase)?;
    let tx_hash = hex::encode(tx_hash_bytes);
    print.log_explorer_url(&network, &tx_hash);

    resources.print_cost_info(&res)?;

    if !no_cache {
        data::write(res.clone().try_into()?, &network.rpc_uri()?)?;
    }

    Ok(res)
}
