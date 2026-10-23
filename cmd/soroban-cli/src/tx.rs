use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::tx::fetch,
    config::{self, data, network},
    print, resources,
    signer::{self, Signer},
    utils::transaction_env_hash,
    xdr::{
        self, FeeBumpTransaction, FeeBumpTransactionExt, FeeBumpTransactionInnerTx, Hash,
        Transaction, TransactionEnvelope,
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
/// On failure, logs the transaction's diagnostic events to stderr (via
/// `send_transaction_polling_with_events`); it does not otherwise log the
/// result or effects of a successful transaction.
///
/// Returns the `GetTransactionResponse` from the network.
///
/// # Errors
/// If any step of the process fails (simulation, signing, sending)
#[allow(clippy::too_many_arguments)]
pub async fn sim_sign_and_send_tx<E>(
    client: &soroban_rpc::Client,
    tx: &Transaction,
    config: &config::Args,
    resources: &resources::Args,
    auth_signers: &[Signer],
    auth_mode: Option<soroban_rpc::AuthMode>,
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
        auth_mode,
    )
    .await?;
    let assembled = resources.apply_to_assembled_txn(txn);
    let mut txn = Box::new(assembled.transaction().clone());
    let sim_res = assembled.sim_response();

    if !no_cache {
        data::write(sim_res.clone().into(), &network.rpc_uri()?)?;
    }

    // Sign all auth entries. Each entry is validated against the transaction's
    // host function inside `sign_soroban_authorizations` before being signed.
    if let Some(tx) = config
        .sign_soroban_authorizations(&txn, auth_signers, &print)
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
    let res = send_transaction_polling_with_events(
        client,
        &signed_tx,
        &network.network_passphrase,
        &print,
    )
    .await?;

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

/// Submits a signed transaction and polls for its result, surfacing diagnostic
/// events when the transaction fails.
///
/// On failure the RPC client returns only the (decoded) result codes and discards
/// the transaction's diagnostic events. This helper best-effort re-fetches the
/// failed transaction to recover and print those events — the host/contract error,
/// its message, and any contract logs — which explain *why* the transaction
/// failed. The original error is always preserved and returned unchanged.
///
/// # Errors
/// Returns the underlying submission error if the transaction fails.
pub async fn send_transaction_polling_with_events(
    client: &soroban_rpc::Client,
    signed_tx: &TransactionEnvelope,
    network_passphrase: &str,
    print: &print::Print,
) -> Result<GetTransactionResponse, soroban_rpc::Error> {
    match client.send_transaction_polling(signed_tx).await {
        Ok(res) => Ok(res),
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("TxInsufficientBalance")
                || err_str.contains("tx_insufficient_balance")
                || err_str.contains("txINSUFFICIENT_BALANCE")
                || err_str.contains("TxInsufficientFee")
                || err_str.contains("tx_insufficient_fee")
            {
                if let Some((source_account, fee)) = extract_envelope_source_and_fee(signed_tx) {
                    check_and_print_insufficient_balance_for_account(
                        client,
                        &source_account,
                        fee,
                        print,
                    )
                    .await;
                }
            }

            // Best-effort: recover the diagnostic events the submission error dropped.
            //
            // Since protocol 23 a failed transaction's diagnostic events are
            // returned in the response's dedicated `diagnosticEventsXdr` field
            // rather than embedded in the transaction meta, so read them from
            // `events.diagnostic_events` (which the RPC client populates from that
            // field) instead of from `result_meta`.
            if let Ok(hash) = transaction_env_hash(signed_tx, network_passphrase) {
                if let Ok(resp) = client.get_transaction(&Hash(hash)).await {
                    let events = &resp.events.diagnostic_events;
                    crate::log::event::all(events);
                    crate::log::event::failure(events, print);
                }
            }
            Err(e)
        }
    }
}

pub fn extract_envelope_source_and_fee(
    tx_env: &TransactionEnvelope,
) -> Option<(xdr::MuxedAccount, i64)> {
    match tx_env {
        TransactionEnvelope::Tx(v1) => Some((v1.tx.source_account.clone(), v1.tx.fee as i64)),
        TransactionEnvelope::TxFeeBump(fb) => Some((fb.tx.fee_source.clone(), fb.tx.fee)),
        _ => None,
    }
}

pub fn format_muxed_account_address(source: &xdr::MuxedAccount) -> String {
    let bytes = match source {
        xdr::MuxedAccount::Ed25519(xdr::Uint256(b)) => *b,
        xdr::MuxedAccount::MuxedEd25519(m) => m.ed25519.0,
    };
    stellar_strkey::ed25519::PublicKey(bytes)
        .to_string()
        .as_str()
        .to_string()
}

pub fn calculate_insufficient_balance_info(
    current_balance: i64,
    fee_needed: i64,
    num_sub_entries: u32,
) -> (i64, i64, Option<i64>) {
    let base_reserve: i64 = 5_000_000;
    let min_reserve = (2 + num_sub_entries as i64) * base_reserve;
    let total_required = fee_needed + min_reserve;
    let shortfall = if current_balance < total_required {
        Some(total_required - current_balance)
    } else {
        None
    };
    (min_reserve, total_required, shortfall)
}

pub async fn check_and_print_insufficient_balance_for_account(
    client: &soroban_rpc::Client,
    source_account: &xdr::MuxedAccount,
    fee_needed: i64,
    print: &print::Print,
) {
    let address = format_muxed_account_address(source_account);
    if let Ok(account) = client.get_account(&address).await {
        let (min_reserve, _, shortfall) = calculate_insufficient_balance_info(
            account.balance,
            fee_needed,
            account.num_sub_entries,
        );

        print.infoln(format!("Fee source account: {address}"));
        print.infoln(format!(
            "Current balance: {} XLM",
            print::format_number(account.balance, 7)
        ));
        print.infoln(format!(
            "Transaction fee required: {} XLM",
            print::format_number(fee_needed, 7)
        ));
        print.infoln(format!(
            "Minimum reserve required: {} XLM (sub-entries: {})",
            print::format_number(min_reserve, 7),
            account.num_sub_entries
        ));

        if let Some(shortfall_amount) = shortfall {
            print.errorln(format!(
                "Insufficient balance: need at least {} more XLM to pay fee and maintain reserve",
                print::format_number(shortfall_amount, 7)
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        Memo, MuxedAccount, Preconditions, SequenceNumber, Transaction, TransactionEnvelope,
        TransactionV1Envelope, Uint256, VecM,
    };

    #[test]
    fn test_calculate_insufficient_balance_info() {
        // Balance 10 XLM (100_000_000 stroops), Fee 2 XLM (20_000_000 stroops), 0 sub-entries (reserve 1 XLM = 10_000_000)
        let (min_reserve, total, shortfall) =
            calculate_insufficient_balance_info(100_000_000, 20_000_000, 0);
        assert_eq!(min_reserve, 10_000_000);
        assert_eq!(total, 30_000_000);
        assert_eq!(shortfall, None);

        // Balance 0.5 XLM (5_000_000 stroops), Fee 5 XLM (50_000_000 stroops), 0 sub-entries (reserve 1 XLM = 10_000_000)
        // total required = 60_000_000, shortfall = 55_000_000 (5.5 XLM)
        let (min_reserve, total, shortfall) =
            calculate_insufficient_balance_info(5_000_000, 50_000_000, 0);
        assert_eq!(min_reserve, 10_000_000);
        assert_eq!(total, 60_000_000);
        assert_eq!(shortfall, Some(55_000_000));

        // Balance 2 XLM (20_000_000 stroops), Fee 1.5 XLM (15_000_000 stroops), 1 sub-entry (reserve = (2+1)*0.5 = 1.5 XLM = 15_000_000)
        // total required = 30_000_000, shortfall = 10_000_000 (1 XLM)
        let (min_reserve, total, shortfall) =
            calculate_insufficient_balance_info(20_000_000, 15_000_000, 1);
        assert_eq!(min_reserve, 15_000_000);
        assert_eq!(total, 30_000_000);
        assert_eq!(shortfall, Some(10_000_000));
    }

    #[test]
    fn test_extract_envelope_source_and_fee() {
        let pk = Uint256([5; 32]);
        let source = MuxedAccount::Ed25519(pk);
        let tx = Transaction {
            source_account: source.clone(),
            fee: 12345,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: VecM::default(),
            ext: xdr::TransactionExt::V0,
        };
        let env = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx,
            signatures: VecM::default(),
        });

        let (extracted_source, extracted_fee) = extract_envelope_source_and_fee(&env).unwrap();
        assert_eq!(extracted_source, source);
        assert_eq!(extracted_fee, 12345);

        let address = format_muxed_account_address(&extracted_source);
        assert!(address.starts_with("G"));
    }
}
