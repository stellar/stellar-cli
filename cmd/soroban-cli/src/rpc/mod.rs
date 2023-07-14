use http::{uri::Authority, Uri};
use itertools::Itertools;
use jsonrpsee_core::params::ObjectParams;
use jsonrpsee_core::{self, client::ClientT, rpc_params};
use jsonrpsee_http_client::{HeaderMap, HttpClient, HttpClientBuilder};
use serde_aux::prelude::{deserialize_default_from_null, deserialize_number_from_string};
use soroban_env_host::{
    budget::Budget,
    events::HostEvent,
    xdr::{
        self, AccountEntry, AccountId, ContractDataEntry, DiagnosticEvent, Error as XdrError,
        LedgerEntryData, LedgerFootprint, LedgerKey, LedgerKeyAccount, PublicKey, ReadXdr,
        SorobanAuthorizationEntry, Transaction, TransactionEnvelope, TransactionMeta,
        TransactionMetaV3, TransactionResult, TransactionV1Envelope, Uint256, VecM, WriteXdr,
    },
};
use soroban_sdk::token;
use std::{
    fmt::Display,
    str::FromStr,
    time::{Duration, Instant},
};
use termcolor::{Color, ColorChoice, StandardStream, WriteColor};
use termcolor_output::colored;
use tokio::time::sleep;

use crate::utils::{self, contract_spec};

mod transaction;
use transaction::assemble;

const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");

pub type LogEvents = fn(
    footprint: &LedgerFootprint,
    auth: &[VecM<SorobanAuthorizationEntry>],
    events: &[HostEvent],
    budget: Option<&Budget>,
) -> ();

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid address: {0}")]
    InvalidAddress(#[from] stellar_strkey::DecodeError),
    #[error("invalid response from server")]
    InvalidResponse,
    #[error("provided network passphrase {expected:?} does not match the server: {server:?}")]
    InvalidNetworkPassphrase { expected: String, server: String },
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("invalid rpc url: {0}")]
    InvalidRpcUrl(http::uri::InvalidUri),
    #[error("invalid rpc url: {0}")]
    InvalidRpcUrlFromUriParts(http::uri::InvalidUriParts),
    #[error("jsonrpc error: {0}")]
    JsonRpc(#[from] jsonrpsee_core::Error),
    #[error("json decoding error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("transaction failed: {0}")]
    TransactionFailed(String),
    #[error("transaction submission failed: {0}")]
    TransactionSubmissionFailed(String),
    #[error("expected transaction status: {0}")]
    UnexpectedTransactionStatus(String),
    #[error("transaction submission timeout")]
    TransactionSubmissionTimeout,
    #[error("transaction simulation failed: {0}")]
    TransactionSimulationFailed(String),
    #[error("{0} not found: {1}")]
    NotFound(String, String),
    #[error("Missing result in successful response")]
    MissingResult,
    #[error("Failed to read Error response from server")]
    MissingError,
    #[error("cursor is not valid")]
    InvalidCursor,
    #[error("unexpected ({length}) simulate transaction result length")]
    UnexpectedSimulateTransactionResultSize { length: usize },
    #[error("unexpected ({count}) number of operations")]
    UnexpectedOperationCount { count: usize },
    #[error("Transaction contains unsupported operation type")]
    UnsupportedOperationType,
    #[error("unexpected contract code data type: {0:?}")]
    UnexpectedContractCodeDataType(LedgerEntryData),
    #[error(transparent)]
    CouldNotParseContractSpec(#[from] contract_spec::Error),
    #[error("unexpected contract code got token")]
    UnexpectedToken(ContractDataEntry),
    #[error(transparent)]
    Spec(#[from] soroban_spec::read::FromWasmError),
    #[error(transparent)]
    SpecBase64(#[from] soroban_spec::read::ParseSpecBase64Error),
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct SendTransactionResponse {
    pub hash: String,
    pub status: String,
    #[serde(
        rename = "errorResultXdr",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub error_result_xdr: Option<String>,
    #[serde(
        rename = "latestLedger",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub latest_ledger: u32,
    #[serde(
        rename = "latestLedgerCloseTime",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub latest_ledger_close_time: u32,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct GetTransactionResponse {
    pub status: String,
    #[serde(
        rename = "envelopeXdr",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub envelope_xdr: Option<String>,
    #[serde(rename = "resultXdr", skip_serializing_if = "Option::is_none", default)]
    pub result_xdr: Option<String>,
    #[serde(
        rename = "resultMetaXdr",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub result_meta_xdr: Option<String>,
    // TODO: add ledger info and application order
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct LedgerEntryResult {
    pub key: String,
    pub xdr: String,
    #[serde(rename = "lastModifiedLedgerSeq")]
    pub last_modified_ledger: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct GetLedgerEntriesResponse {
    pub entries: Option<Vec<LedgerEntryResult>>,
    #[serde(rename = "latestLedger")]
    pub latest_ledger: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct GetNetworkResponse {
    #[serde(
        rename = "friendbotUrl",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub friendbot_url: Option<String>,
    pub passphrase: String,
    #[serde(
        rename = "protocolVersion",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub protocol_version: u32,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct Cost {
    #[serde(
        rename = "cpuInsns",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub cpu_insns: String,
    #[serde(
        rename = "memBytes",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub mem_bytes: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct SimulateHostFunctionResult {
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub auth: Vec<String>,
    pub xdr: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct SimulateTransactionResponse {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
    #[serde(rename = "transactionData")]
    pub transaction_data: String,
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub events: Vec<String>,
    #[serde(
        rename = "minResourceFee",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub min_resource_fee: u32,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub results: Vec<SimulateHostFunctionResult>,
    pub cost: Cost,
    #[serde(
        rename = "latestLedger",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub latest_ledger: u32,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct GetEventsResponse {
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub events: Vec<Event>,
    #[serde(
        rename = "latestLedger",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub latest_ledger: u32,
}

// Determines whether or not a particular filter matches a topic based on the
// same semantics as the RPC server:
//
//  - for an exact segment match, the filter is a base64-encoded ScVal
//  - for a wildcard, single-segment match, the string "*" matches exactly one
//    segment
//
// The expectation is that a `filter` is a comma-separated list of segments that
// has previously been validated, and `topic` is the list of segments applicable
// for this event.
//
// [API
// Reference](https://docs.google.com/document/d/1TZUDgo_3zPz7TiPMMHVW_mtogjLyPL0plvzGMsxSz6A/edit#bookmark=id.35t97rnag3tx)
// [Code
// Reference](https://github.com/stellar/soroban-tools/blob/bac1be79e8c2590c9c35ad8a0168aab0ae2b4171/cmd/soroban-rpc/internal/methods/get_events.go#L182-L203)
pub fn does_topic_match(topic: &[String], filter: &[String]) -> bool {
    filter.len() == topic.len()
        && filter
            .iter()
            .enumerate()
            .all(|(i, s)| *s == "*" || topic[i] == *s)
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Event {
    #[serde(rename = "type")]
    pub event_type: String,

    pub ledger: String,
    #[serde(rename = "ledgerClosedAt")]
    pub ledger_closed_at: String,

    pub id: String,
    #[serde(rename = "pagingToken")]
    pub paging_token: String,

    #[serde(rename = "contractId")]
    pub contract_id: String,
    pub topic: Vec<String>,
    pub value: EventValue,
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Event {} [{}]:",
            self.paging_token,
            self.event_type.to_ascii_uppercase()
        )?;
        writeln!(
            f,
            "  Ledger:   {} (closed at {})",
            self.ledger, self.ledger_closed_at
        )?;
        writeln!(f, "  Contract: {}", self.contract_id)?;
        writeln!(f, "  Topics:")?;
        for topic in &self.topic {
            let scval = xdr::ScVal::from_xdr_base64(topic).map_err(|_| std::fmt::Error)?;
            writeln!(f, "            {scval:?}")?;
        }
        let scval = xdr::ScVal::from_xdr_base64(&self.value.xdr).map_err(|_| std::fmt::Error)?;
        writeln!(f, "  Value:    {scval:?}")
    }
}

impl Event {
    pub fn parse_cursor(&self) -> Result<(u64, i32), Error> {
        parse_cursor(&self.id)
    }

    pub fn pretty_print(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut stdout = StandardStream::stdout(ColorChoice::Auto);
        if !stdout.supports_color() {
            println!("{self}");
            return Ok(());
        }

        let color = match self.event_type.as_str() {
            "system" => Color::Yellow,
            _ => Color::Blue,
        };
        colored!(
            stdout,
            "{}Event{} {}{}{} [{}{}{}{}]:\n",
            bold!(true),
            bold!(false),
            fg!(Some(Color::Green)),
            self.paging_token,
            reset!(),
            bold!(true),
            fg!(Some(color)),
            self.event_type.to_ascii_uppercase(),
            reset!(),
        )?;

        colored!(
            stdout,
            "  Ledger:   {}{}{} (closed at {}{}{})\n",
            fg!(Some(Color::Green)),
            self.ledger,
            reset!(),
            fg!(Some(Color::Green)),
            self.ledger_closed_at,
            reset!(),
        )?;

        colored!(
            stdout,
            "  Contract: {}0x{}{}\n",
            fg!(Some(Color::Green)),
            self.contract_id,
            reset!(),
        )?;

        colored!(stdout, "  Topics:\n")?;
        for topic in &self.topic {
            let scval = xdr::ScVal::from_xdr_base64(topic)?;
            colored!(
                stdout,
                "            {}{:?}{}\n",
                fg!(Some(Color::Green)),
                scval,
                reset!(),
            )?;
        }

        let scval = xdr::ScVal::from_xdr_base64(&self.value.xdr)?;
        colored!(
            stdout,
            "  Value: {}{:?}{}\n",
            fg!(Some(Color::Green)),
            scval,
            reset!(),
        )?;

        Ok(())
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct EventValue {
    pub xdr: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum EventType {
    All,
    Contract,
    System,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EventStart {
    Ledger(u32),
    Cursor(String),
}

pub struct Client {
    base_url: String,
}

impl Client {
    pub fn new(base_url: &str) -> Result<Self, Error> {
        // Add the port to the base URL if there is no port explicitly included
        // in the URL and the scheme allows us to infer a default port.
        // Jsonrpsee requires a port to always be present even if one can be
        // inferred. This may change: https://github.com/paritytech/jsonrpsee/issues/1048.
        let uri = base_url.parse::<Uri>().map_err(Error::InvalidRpcUrl)?;
        let mut parts = uri.into_parts();
        if let (Some(scheme), Some(authority)) = (&parts.scheme, &parts.authority) {
            if authority.port().is_none() {
                let port = match scheme.as_str() {
                    "http" => Some(80),
                    "https" => Some(443),
                    _ => None,
                };
                if let Some(port) = port {
                    let host = authority.host();
                    parts.authority = Some(
                        Authority::from_str(&format!("{host}:{port}"))
                            .map_err(Error::InvalidRpcUrl)?,
                    );
                }
            }
        }
        let uri = Uri::from_parts(parts).map_err(Error::InvalidRpcUrlFromUriParts)?;
        tracing::trace!(?uri);
        Ok(Self {
            base_url: uri.to_string(),
        })
    }

    fn client(&self) -> Result<HttpClient, Error> {
        let url = self.base_url.clone();
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-Name", "soroban-cli".parse().unwrap());
        let version = VERSION.unwrap_or("devel");
        headers.insert("X-Client-Version", version.parse().unwrap());
        Ok(HttpClientBuilder::default()
            .set_headers(headers)
            .build(url)?)
    }

    pub async fn verify_network_passphrase(&self, expected: Option<&str>) -> Result<String, Error> {
        let server = self.get_network().await?.passphrase;
        if expected.is_some() && expected != Some(&server) {
            return Err(Error::InvalidNetworkPassphrase {
                expected: expected.unwrap().to_string(),
                server,
            });
        }
        Ok(server)
    }

    pub async fn get_network(&self) -> Result<GetNetworkResponse, Error> {
        tracing::trace!("Getting network");
        Ok(self.client()?.request("getNetwork", rpc_params![]).await?)
    }

    pub async fn get_account(&self, address: &str) -> Result<AccountEntry, Error> {
        tracing::trace!("Getting address {}", address);
        let key = LedgerKey::Account(LedgerKeyAccount {
            account_id: AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
                stellar_strkey::ed25519::PublicKey::from_string(address)?.0,
            ))),
        });
        let keys = Vec::from([key]);
        let response = self.get_ledger_entries(keys).await?;
        let entries = response.entries.unwrap_or_default();
        if entries.is_empty() {
            return Err(Error::NotFound("Account".to_string(), address.to_string()));
        }
        let ledger_entry = &entries[0];
        if let LedgerEntryData::Account(entry) =
            LedgerEntryData::read_xdr_base64(&mut ledger_entry.xdr.as_bytes())?
        {
            tracing::trace!(account=?entry);
            Ok(entry)
        } else {
            Err(Error::InvalidResponse)
        }
    }

    pub async fn send_transaction(
        &self,
        tx: &TransactionEnvelope,
    ) -> Result<(TransactionResult, TransactionMeta, Vec<DiagnosticEvent>), Error> {
        let client = self.client()?;
        tracing::trace!(?tx);
        let SendTransactionResponse {
            hash,
            error_result_xdr,
            status,
            ..
        } = client
            .request("sendTransaction", rpc_params![tx.to_xdr_base64()?])
            .await
            .map_err(|err| Error::TransactionSubmissionFailed(format!("{err:#?}")))?;

        if status == "ERROR" {
            let error = error_result_xdr
                .ok_or(Error::MissingError)
                .and_then(|x| {
                    TransactionResult::read_xdr_base64(&mut x.as_bytes())
                        .map_err(|_| Error::InvalidResponse)
                })
                .map(|r| r.result);
            tracing::error!(?error);
            return Err(Error::TransactionSubmissionFailed(format!("{:#?}", error?)));
        }
        // even if status == "success" we need to query the transaction status in order to get the result

        // Poll the transaction status
        let start = Instant::now();
        loop {
            let response = self.get_transaction(&hash).await?;
            match response.status.as_str() {
                "SUCCESS" => {
                    // TODO: the caller should probably be printing this
                    tracing::trace!(?response);
                    let result = TransactionResult::from_xdr_base64(
                        response.result_xdr.clone().ok_or(Error::MissingResult)?,
                    )?;
                    let meta = TransactionMeta::from_xdr_base64(
                        response
                            .result_meta_xdr
                            .clone()
                            .ok_or(Error::MissingResult)?,
                    )?;
                    let events = extract_events(&meta);
                    return Ok((result, meta, events));
                }
                "FAILED" => {
                    tracing::error!(?response);
                    // TODO: provide a more elaborate error
                    return Err(Error::TransactionSubmissionFailed(format!("{response:#?}")));
                }
                "NOT_FOUND" => (),
                _ => {
                    return Err(Error::UnexpectedTransactionStatus(response.status));
                }
            };
            let duration = start.elapsed();
            // TODO: parameterize the timeout instead of using a magic constant
            if duration.as_secs() > 10 {
                return Err(Error::TransactionSubmissionTimeout);
            }
            sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn simulate_transaction(
        &self,
        tx: &TransactionEnvelope,
    ) -> Result<SimulateTransactionResponse, Error> {
        tracing::trace!(?tx);
        let base64_tx = tx.to_xdr_base64()?;
        let response: SimulateTransactionResponse = self
            .client()?
            .request("simulateTransaction", rpc_params![base64_tx])
            .await?;
        tracing::trace!(?response);
        match response.error {
            None => Ok(response),
            Some(e) => Err(Error::TransactionSimulationFailed(e)),
        }
    }

    // Simulate a transaction, then assemble the result of the simulation into the envelope, so it
    // is ready for sending to the network.
    pub async fn prepare_transaction(
        &self,
        tx: &Transaction,
        log_events: Option<LogEvents>,
    ) -> Result<Transaction, Error> {
        tracing::trace!(?tx);
        let sim_response = self
            .simulate_transaction(&TransactionEnvelope::Tx(TransactionV1Envelope {
                tx: tx.clone(),
                signatures: VecM::default(),
            }))
            .await?;
        assemble(tx, &sim_response, log_events)
    }

    pub async fn prepare_and_send_transaction(
        &self,
        tx_without_preflight: &Transaction,
        key: &ed25519_dalek::Keypair,
        network_passphrase: &str,
        log_events: Option<LogEvents>,
    ) -> Result<(TransactionResult, TransactionMeta, Vec<DiagnosticEvent>), Error> {
        let unsigned_tx = self
            .prepare_transaction(tx_without_preflight, log_events)
            .await?;
        let tx = utils::sign_transaction(key, &unsigned_tx, network_passphrase)?;
        self.send_transaction(&tx).await
    }

    pub async fn get_transaction(&self, tx_id: &str) -> Result<GetTransactionResponse, Error> {
        Ok(self
            .client()?
            .request("getTransaction", rpc_params![tx_id])
            .await?)
    }

    pub async fn get_ledger_entries(
        &self,
        keys: Vec<LedgerKey>,
    ) -> Result<GetLedgerEntriesResponse, Error> {
        let mut base64_keys: Vec<String> = vec![];
        for k in &keys {
            let base64_result = k.to_xdr_base64();
            if base64_result.is_err() {
                return Err(Error::Xdr(XdrError::Invalid));
            }
            base64_keys.push(k.to_xdr_base64().unwrap());
        }
        Ok(self
            .client()?
            .request("getLedgerEntries", rpc_params![base64_keys])
            .await?)
    }

    pub async fn get_events(
        &self,
        start: EventStart,
        event_type: Option<EventType>,
        contract_ids: &[String],
        topics: &[String],
        limit: Option<usize>,
    ) -> Result<GetEventsResponse, Error> {
        let mut filters = serde_json::Map::new();

        event_type
            .and_then(|t| match t {
                EventType::All => None, // all is the default, so avoid incl. the param
                EventType::Contract => Some("contract"),
                EventType::System => Some("system"),
            })
            .map(|t| filters.insert("type".to_string(), t.into()));

        filters.insert("topics".to_string(), topics.into());
        filters.insert("contractIds".to_string(), contract_ids.into());

        let mut pagination = serde_json::Map::new();
        if let Some(limit) = limit {
            pagination.insert("limit".to_string(), limit.into());
        }

        let mut oparams = ObjectParams::new();
        match start {
            EventStart::Ledger(l) => oparams.insert("startLedger", l.to_string())?,
            EventStart::Cursor(c) => {
                pagination.insert("cursor".to_string(), c.into());
            }
        };
        oparams.insert("filters", vec![filters])?;
        oparams.insert("pagination", pagination)?;

        Ok(self.client()?.request("getEvents", oparams).await?)
    }

    pub async fn get_contract_data(
        &self,
        contract_id: &[u8; 32],
    ) -> Result<ContractDataEntry, Error> {
        // Get the contract from the network
        let contract_key = LedgerKey::ContractData(xdr::LedgerKeyContractData {
            contract: xdr::ScAddress::Contract(xdr::Hash(*contract_id)),
            key: xdr::ScVal::LedgerKeyContractInstance,
            durability: xdr::ContractDataDurability::Persistent,
            body_type: xdr::ContractEntryBodyType::DataEntry,
        });
        let contract_ref = self.get_ledger_entries(Vec::from([contract_key])).await?;
        let entries = contract_ref.entries.unwrap_or_default();
        if entries.is_empty() {
            let contract_address = stellar_strkey::Contract(*contract_id).to_string();
            return Err(Error::NotFound("Contract".to_string(), contract_address));
        }
        let contract_ref_entry = &entries[0];
        match LedgerEntryData::from_xdr_base64(&contract_ref_entry.xdr)? {
            LedgerEntryData::ContractData(contract_data) => Ok(contract_data),
            scval => Err(Error::UnexpectedContractCodeDataType(scval)),
        }
    }

    pub async fn get_remote_wasm(&self, contract_id: &[u8; 32]) -> Result<Vec<u8>, Error> {
        match self.get_contract_data(contract_id).await? {
            xdr::ContractDataEntry {
                body:
                    xdr::ContractDataEntryBody::DataEntry(xdr::ContractDataEntryData {
                        val:
                            xdr::ScVal::ContractInstance(xdr::ScContractInstance {
                                executable: xdr::ContractExecutable::Wasm(hash),
                                ..
                            }),
                        ..
                    }),
                ..
            } => self.get_remote_wasm_from_hash(hash).await,
            scval => Err(Error::UnexpectedToken(scval)),
        }
    }

    pub async fn get_remote_wasm_from_hash(&self, hash: xdr::Hash) -> Result<Vec<u8>, Error> {
        let code_key = LedgerKey::ContractCode(xdr::LedgerKeyContractCode {
            hash: hash.clone(),
            body_type: xdr::ContractEntryBodyType::DataEntry,
        });
        let contract_data = self.get_ledger_entries(Vec::from([code_key])).await?;
        let entries = contract_data.entries.unwrap_or_default();
        if entries.is_empty() {
            return Err(Error::NotFound(
                "Contract Code".to_string(),
                hex::encode(hash),
            ));
        }
        let contract_data_entry = &entries[0];
        match LedgerEntryData::from_xdr_base64(&contract_data_entry.xdr)? {
            LedgerEntryData::ContractCode(xdr::ContractCodeEntry {
                body: xdr::ContractCodeEntryBody::DataEntry(code),
                ..
            }) => Ok(code.into()),
            scval => Err(Error::UnexpectedContractCodeDataType(scval)),
        }
    }

    pub async fn get_remote_contract_spec(
        &self,
        contract_id: &[u8; 32],
    ) -> Result<Vec<xdr::ScSpecEntry>, Error> {
        let contract_data = self.get_contract_data(contract_id).await?;
        let xdr::ContractDataEntryBody::DataEntry(data) = contract_data.body else {
            return Err(Error::Xdr(XdrError::Invalid));
        };
        match data.val {
            xdr::ScVal::ContractInstance(xdr::ScContractInstance {
                executable: xdr::ContractExecutable::Wasm(hash),
                ..
            }) => Ok(contract_spec::ContractSpec::new(
                &self.get_remote_wasm_from_hash(hash).await?,
            )
            .map_err(Error::CouldNotParseContractSpec)?
            .spec),
            xdr::ScVal::ContractInstance(xdr::ScContractInstance {
                executable: xdr::ContractExecutable::Token,
                ..
            }) => Ok(soroban_spec::read::parse_raw(
                &token::StellarAssetSpec::spec_xdr(),
            )?),
            _ => Err(Error::Xdr(XdrError::Invalid)),
        }
    }
}

fn extract_events(tx_meta: &TransactionMeta) -> Vec<DiagnosticEvent> {
    match tx_meta {
        TransactionMeta::V3(TransactionMetaV3 {
            soroban_meta: Some(meta),
            ..
        }) => {
            // NOTE: we assume there can only be one operation, since we only send one
            if meta.diagnostic_events.len() == 1 {
                meta.diagnostic_events.clone().into()
            } else if meta.events.len() == 1 {
                meta.events
                    .iter()
                    .map(|e| DiagnosticEvent {
                        in_successful_contract_call: true,
                        event: e.clone(),
                    })
                    .collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

pub fn parse_cursor(c: &str) -> Result<(u64, i32), Error> {
    let (toid_part, event_index) = c.split('-').collect_tuple().ok_or(Error::InvalidCursor)?;
    let toid_part: u64 = toid_part.parse().map_err(|_| Error::InvalidCursor)?;
    let start_index: i32 = event_index.parse().map_err(|_| Error::InvalidCursor)?;
    Ok((toid_part, start_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_url_default_ports() {
        // Default ports are added.
        let client = Client::new("http://example.com").unwrap();
        assert_eq!(client.base_url, "http://example.com:80/");
        let client = Client::new("https://example.com").unwrap();
        assert_eq!(client.base_url, "https://example.com:443/");

        // Ports are not added when already present.
        let client = Client::new("http://example.com:8080").unwrap();
        assert_eq!(client.base_url, "http://example.com:8080/");
        let client = Client::new("https://example.com:8080").unwrap();
        assert_eq!(client.base_url, "https://example.com:8080/");

        // Paths are not modified.
        let client = Client::new("http://example.com/a/b/c").unwrap();
        assert_eq!(client.base_url, "http://example.com:80/a/b/c");
        let client = Client::new("https://example.com/a/b/c").unwrap();
        assert_eq!(client.base_url, "https://example.com:443/a/b/c");
        let client = Client::new("http://example.com/a/b/c/").unwrap();
        assert_eq!(client.base_url, "http://example.com:80/a/b/c/");
        let client = Client::new("https://example.com/a/b/c/").unwrap();
        assert_eq!(client.base_url, "https://example.com:443/a/b/c/");
        let client = Client::new("http://example.com/a/b:80/c/").unwrap();
        assert_eq!(client.base_url, "http://example.com:80/a/b:80/c/");
        let client = Client::new("https://example.com/a/b:80/c/").unwrap();
        assert_eq!(client.base_url, "https://example.com:443/a/b:80/c/");
    }

    #[test]
    // Taken from [RPC server
    // tests](https://github.com/stellar/soroban-tools/blob/main/cmd/soroban-rpc/internal/methods/get_events_test.go#L21).
    fn test_does_topic_match() {
        struct TestCase<'a> {
            name: &'a str,
            filter: Vec<&'a str>,
            includes: Vec<Vec<&'a str>>,
            excludes: Vec<Vec<&'a str>>,
        }

        let xfer = "AAAABQAAAAh0cmFuc2Zlcg==";
        let number = "AAAAAQB6Mcc=";
        let star = "*";

        for tc in vec![
            // No filter means match nothing.
            TestCase {
                name: "<empty>",
                filter: vec![],
                includes: vec![],
                excludes: vec![vec![xfer]],
            },
            // "*" should match "transfer/" but not "transfer/transfer" or
            // "transfer/amount", because * is specified as a SINGLE segment
            // wildcard.
            TestCase {
                name: "*",
                filter: vec![star],
                includes: vec![vec![xfer]],
                excludes: vec![vec![xfer, xfer], vec![xfer, number]],
            },
            // "*/transfer" should match anything preceding "transfer", but
            // nothing that isn't exactly two segments long.
            TestCase {
                name: "*/transfer",
                filter: vec![star, xfer],
                includes: vec![vec![number, xfer], vec![xfer, xfer]],
                excludes: vec![
                    vec![number],
                    vec![number, number],
                    vec![number, xfer, number],
                    vec![xfer],
                    vec![xfer, number],
                    vec![xfer, xfer, xfer],
                ],
            },
            // The inverse case of before: "transfer/*" should match any single
            // segment after a segment that is exactly "transfer", but no
            // additional segments.
            TestCase {
                name: "transfer/*",
                filter: vec![xfer, star],
                includes: vec![vec![xfer, number], vec![xfer, xfer]],
                excludes: vec![
                    vec![number],
                    vec![number, number],
                    vec![number, xfer, number],
                    vec![xfer],
                    vec![number, xfer],
                    vec![xfer, xfer, xfer],
                ],
            },
            // Here, we extend to exactly two wild segments after transfer.
            TestCase {
                name: "transfer/*/*",
                filter: vec![xfer, star, star],
                includes: vec![vec![xfer, number, number], vec![xfer, xfer, xfer]],
                excludes: vec![
                    vec![number],
                    vec![number, number],
                    vec![number, xfer],
                    vec![number, xfer, number, number],
                    vec![xfer],
                    vec![xfer, xfer, xfer, xfer],
                ],
            },
            // Here, we ensure wildcards can be in the middle of a filter: only
            // exact matches happen on the ends, while the middle can be
            // anything.
            TestCase {
                name: "transfer/*/number",
                filter: vec![xfer, star, number],
                includes: vec![vec![xfer, number, number], vec![xfer, xfer, number]],
                excludes: vec![
                    vec![number],
                    vec![number, number],
                    vec![number, number, number],
                    vec![number, xfer, number],
                    vec![xfer],
                    vec![number, xfer],
                    vec![xfer, xfer, xfer],
                    vec![xfer, number, xfer],
                ],
            },
        ] {
            for topic in tc.includes {
                assert!(
                    does_topic_match(
                        &topic
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect::<Vec<String>>(),
                        &tc.filter
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect::<Vec<String>>()
                    ),
                    "test: {}, topic ({:?}) should be matched by filter ({:?})",
                    tc.name,
                    topic,
                    tc.filter
                );
            }

            for topic in tc.excludes {
                assert!(
                    !does_topic_match(
                        // make deep copies of the vecs
                        &topic
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect::<Vec<String>>(),
                        &tc.filter
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect::<Vec<String>>()
                    ),
                    "test: {}, topic ({:?}) should NOT be matched by filter ({:?})",
                    tc.name,
                    topic,
                    tc.filter
                );
            }
        }
    }
}
