use std::time::Duration;

use crate::rpc::{Client, GetTransactionsRequest};
use chrono::Utc;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::commands::watch::decode::decode_scval;
use crate::commands::watch::error::Result;

#[derive(serde::Deserialize)]
struct RawEvent {
    #[serde(flatten)]
    inner: crate::rpc::Event,
    #[serde(rename = "txHash", default)]
    tx_hash: String,
}

#[derive(serde::Deserialize)]
struct RawEventsResult {
    events: Vec<RawEvent>,
    #[serde(rename = "latestLedger")]
    latest_ledger: u32,
    #[serde(rename = "oldestLedger")]
    oldest_ledger: u32,
}

#[derive(serde::Deserialize)]
struct JsonRpcResp {
    result: RawEventsResult,
}

async fn fetch_events_raw(
    client: &reqwest::Client,
    rpc_url: &str,
    start_ledger: u32,
    limit: u32,
) -> Result<RawEventsResult> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getEvents",
        "params": {
            "startLedger": start_ledger,
            "filters": [],
            "pagination": { "limit": limit }
        }
    });
    let resp = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("getEvents HTTP: {e}"))?
        .json::<JsonRpcResp>()
        .await
        .map_err(|e| format!("getEvents parse: {e}"))?;
    Ok(resp.result)
}

use crate::commands::watch::event::{
    AppEvent, ConnectionStatus, EventData, EventKind, TransactionData, WorkerMessage,
};

pub async fn run_rpc_supervisor(
    rpc_url: String,
    poll_interval: u64,
    tx: mpsc::UnboundedSender<WorkerMessage>,
    network_passphrase: String,
) {
    loop {
        let _ = tx.send(WorkerMessage::RpcStatus(ConnectionStatus::Connecting));
        match run_rpc_watcher(&rpc_url, poll_interval, &tx, &network_passphrase).await {
            Ok(()) => {}
            Err(e) => {
                let _ = tx.send(WorkerMessage::RpcStatus(ConnectionStatus::Error(
                    e.to_string(),
                )));
            }
        }
        sleep(Duration::from_secs(5)).await;
    }
}

async fn run_rpc_watcher(
    rpc_url: &str,
    poll_interval: u64,
    tx: &mpsc::UnboundedSender<WorkerMessage>,
    network_passphrase: &str,
) -> Result<()> {
    let http_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let client = Client::new(rpc_url).map_err(|e| format!("RPC client init: {e}"))?;

    let latest = client
        .get_latest_ledger()
        .await
        .map_err(|e| format!("getLatestLedger: {e}"))?;

    let start_ledger = latest.sequence.saturating_sub(2);
    let mut tx_cursor: Option<u64> = None;
    let mut event_ledger = start_ledger;

    let _ = tx.send(WorkerMessage::RpcStatus(ConnectionStatus::Connected));

    loop {
        let tx_request = GetTransactionsRequest {
            start_ledger: if tx_cursor.is_none() {
                Some(start_ledger)
            } else {
                None
            },
            pagination: tx_cursor.map(|c| crate::rpc::TransactionsPaginationOptions {
                cursor: Some(c),
                limit: Some(100),
            }),
        };

        match client.get_transactions(tx_request).await {
            Ok(resp) => {
                for item in &resp.transactions {
                    let event = transaction_to_event(item, network_passphrase);
                    let _ = tx.send(WorkerMessage::NewEvent(event));
                }
                tx_cursor = Some(resp.cursor);
            }
            Err(e) => {
                let _ = tx.send(WorkerMessage::RpcStatus(ConnectionStatus::Error(format!(
                    "getTransactions: {e}"
                ))));
                sleep(Duration::from_secs(poll_interval)).await;
                continue;
            }
        }

        match fetch_events_raw(&http_client, rpc_url, event_ledger, 100).await {
            Ok(resp) => {
                for item in &resp.events {
                    let event = rpc_event_to_app_event(item);
                    let _ = tx.send(WorkerMessage::NewEvent(event));
                }
                if let Some(last) = resp.events.last() {
                    event_ledger = last.inner.ledger;
                } else {
                    event_ledger = resp.latest_ledger;
                }
            }
            Err(e) => {
                let _ = tx.send(WorkerMessage::RpcStatus(ConnectionStatus::Error(format!(
                    "getEvents: {e}"
                ))));
            }
        }

        let _ = tx.send(WorkerMessage::RpcStatus(ConnectionStatus::Connected));
        sleep(Duration::from_secs(poll_interval)).await;
    }
}

pub async fn fetch_older(
    rpc_url: String,
    before_ledger: u32,
    tx: mpsc::UnboundedSender<WorkerMessage>,
    network_passphrase: String,
) {
    let page_size: u32 = 200;
    let start_ledger = before_ledger.saturating_sub(page_size);

    match fetch_older_inner(&rpc_url, start_ledger, before_ledger, &network_passphrase).await {
        Ok((events, oldest_available)) => {
            let _ = tx.send(WorkerMessage::OlderFetched {
                events,
                oldest_available,
            });
        }
        Err(_) => {
            let _ = tx.send(WorkerMessage::OlderFetched {
                events: vec![],
                oldest_available: before_ledger,
            });
        }
    }
}

async fn fetch_older_inner(
    rpc_url: &str,
    start_ledger: u32,
    before_ledger: u32,
    network_passphrase: &str,
) -> Result<(Vec<AppEvent>, u32)> {
    let http_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let client = Client::new(rpc_url).map_err(|e| format!("RPC client init: {e}"))?;
    let mut events = Vec::new();
    let mut oldest_available = start_ledger;

    let tx_request = GetTransactionsRequest {
        start_ledger: Some(start_ledger),
        pagination: Some(crate::rpc::TransactionsPaginationOptions {
            cursor: None,
            limit: Some(200),
        }),
    };
    if let Ok(resp) = client.get_transactions(tx_request).await {
        oldest_available = resp.oldest_ledger;
        for item in &resp.transactions {
            if item.ledger.is_some_and(|l| l < before_ledger) {
                events.push(transaction_to_event(item, network_passphrase));
            }
        }
    }

    if let Ok(resp) = fetch_events_raw(&http_client, rpc_url, start_ledger, 200).await {
        for item in &resp.events {
            if item.inner.ledger < before_ledger {
                events.push(rpc_event_to_app_event(item));
            }
        }
        if resp.oldest_ledger > oldest_available {
            oldest_available = resp.oldest_ledger;
        }
    }

    Ok((events, oldest_available))
}

fn transaction_to_event(
    item: &crate::rpc::GetTransactionResponse,
    network_passphrase: &str,
) -> AppEvent {
    let ledger = item.ledger.unwrap_or(0);

    let (source_account, fee_charged, operation_count, operation_types) =
        extract_tx_envelope_info(item);

    let tx_hash = extract_tx_hash(item, network_passphrase);

    AppEvent {
        id: 0,
        timestamp: Utc::now(),
        kind: EventKind::Transaction(TransactionData {
            tx_hash,
            ledger,
            status: item.status.clone(),
            source_account,
            fee_charged,
            operation_count,
            operation_types,
        }),
    }
}

fn extract_tx_envelope_info(
    item: &crate::rpc::GetTransactionResponse,
) -> (String, i64, u32, Vec<String>) {
    use crate::xdr::{
        FeeBumpTransactionEnvelope, FeeBumpTransactionInnerTx, TransactionEnvelope,
        TransactionV0Envelope, TransactionV1Envelope,
    };

    let Some(envelope) = &item.envelope else {
        return ("unknown".to_string(), 0, 0, vec![]);
    };

    match envelope {
        TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) => {
            let source = format_muxed_account(&tx.source_account);
            let fee = i64::from(tx.fee);
            let count = u32::try_from(tx.operations.len()).unwrap_or(u32::MAX);
            let types = tx
                .operations
                .iter()
                .map(|op| operation_type_name(&op.body))
                .collect();
            (source, fee, count, types)
        }
        TransactionEnvelope::TxV0(TransactionV0Envelope { tx, .. }) => {
            use crate::commands::watch::decode::encode_account_key;
            let source = encode_account_key(tx.source_account_ed25519.0.as_ref());
            let fee = i64::from(tx.fee);
            let count = u32::try_from(tx.operations.len()).unwrap_or(u32::MAX);
            let types = tx
                .operations
                .iter()
                .map(|op| operation_type_name(&op.body))
                .collect();
            (source, fee, count, types)
        }
        TransactionEnvelope::TxFeeBump(FeeBumpTransactionEnvelope { tx, .. }) => {
            let source = format_muxed_account(&tx.fee_source);
            let fee = tx.fee;
            let FeeBumpTransactionInnerTx::Tx(TransactionV1Envelope { tx: inner, .. }) =
                &tx.inner_tx;
            let count = u32::try_from(inner.operations.len()).unwrap_or(u32::MAX);
            let types = inner
                .operations
                .iter()
                .map(|op| operation_type_name(&op.body))
                .collect();
            (source, fee, count, types)
        }
    }
}

fn format_muxed_account(account: &crate::xdr::MuxedAccount) -> String {
    use crate::xdr::MuxedAccount;
    match account {
        MuxedAccount::Ed25519(key) => {
            crate::commands::watch::decode::encode_account_key(key.0.as_ref())
        }
        MuxedAccount::MuxedEd25519(m) => {
            format!("muxed:{}", m.id)
        }
    }
}

fn operation_type_name(body: &crate::xdr::OperationBody) -> String {
    use crate::xdr::OperationBody;
    match body {
        OperationBody::CreateAccount(_) => "create_account",
        OperationBody::Payment(_) => "payment",
        OperationBody::PathPaymentStrictReceive(_) => "path_payment_strict_receive",
        OperationBody::ManageSellOffer(_) => "manage_sell_offer",
        OperationBody::CreatePassiveSellOffer(_) => "create_passive_sell_offer",
        OperationBody::SetOptions(_) => "set_options",
        OperationBody::ChangeTrust(_) => "change_trust",
        OperationBody::AllowTrust(_) => "allow_trust",
        OperationBody::AccountMerge(_) => "account_merge",
        OperationBody::Inflation => "inflation",
        OperationBody::ManageData(_) => "manage_data",
        OperationBody::BumpSequence(_) => "bump_sequence",
        OperationBody::ManageBuyOffer(_) => "manage_buy_offer",
        OperationBody::PathPaymentStrictSend(_) => "path_payment_strict_send",
        OperationBody::CreateClaimableBalance(_) => "create_claimable_balance",
        OperationBody::ClaimClaimableBalance(_) => "claim_claimable_balance",
        OperationBody::BeginSponsoringFutureReserves(_) => "begin_sponsoring_future_reserves",
        OperationBody::EndSponsoringFutureReserves => "end_sponsoring_future_reserves",
        OperationBody::RevokeSponsorship(_) => "revoke_sponsorship",
        OperationBody::Clawback(_) => "clawback",
        OperationBody::ClawbackClaimableBalance(_) => "clawback_claimable_balance",
        OperationBody::SetTrustLineFlags(_) => "set_trustline_flags",
        OperationBody::LiquidityPoolDeposit(_) => "liquidity_pool_deposit",
        OperationBody::LiquidityPoolWithdraw(_) => "liquidity_pool_withdraw",
        OperationBody::InvokeHostFunction(_) => "invoke_contract_function",
        OperationBody::ExtendFootprintTtl(_) => "extend_footprint_ttl",
        OperationBody::RestoreFootprint(_) => "restore_footprint",
    }
    .to_string()
}

fn extract_tx_hash(item: &crate::rpc::GetTransactionResponse, network_passphrase: &str) -> String {
    use crate::xdr::{
        FeeBumpTransactionEnvelope, FeeBumpTransactionInnerTx, Hash, TransactionEnvelope,
        TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
        TransactionV1Envelope, WriteXdr,
    };
    use sha2::{Digest, Sha256};

    let Some(envelope) = &item.envelope else {
        return "unknown".to_string();
    };

    let tx = match envelope {
        TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) => tx.clone(),
        TransactionEnvelope::TxFeeBump(FeeBumpTransactionEnvelope { tx, .. }) => {
            let FeeBumpTransactionInnerTx::Tx(TransactionV1Envelope { tx: inner, .. }) =
                &tx.inner_tx;
            inner.clone()
        }
        TransactionEnvelope::TxV0(_) => return "unknown".to_string(),
    };

    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());
    let payload = TransactionSignaturePayload {
        network_id,
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(tx),
    };

    match payload.to_xdr(crate::xdr::Limits::none()) {
        Ok(bytes) => {
            let hash: [u8; 32] = Sha256::digest(&bytes).into();
            hex::encode(hash)
        }
        Err(_) => "unknown".to_string(),
    }
}

fn rpc_event_to_app_event(item: &RawEvent) -> AppEvent {
    let e = &item.inner;
    let topics: Vec<_> = e.topic.iter().map(|t| decode_scval(t)).collect();
    let value = decode_scval(&e.value);

    let event_type_label = topics
        .first()
        .map_or_else(|| e.event_type.clone(), |t| t.display.clone());

    AppEvent {
        id: 0,
        timestamp: Utc::now(),
        kind: EventKind::Event(EventData {
            event_id: e.id.clone(),
            contract_id: e.contract_id.clone(),
            tx_hash: item.tx_hash.clone(),
            ledger: e.ledger,
            event_type: event_type_label,
            topics,
            value,
            raw_topics: e.topic.clone(),
            raw_value: e.value.clone(),
        }),
    }
}
