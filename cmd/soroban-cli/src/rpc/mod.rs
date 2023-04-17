use http::{uri::Authority, Uri};
use itertools::Itertools;
use jsonrpsee_core::{self, client::ClientT, rpc_params};
use jsonrpsee_http_client::{types, HeaderMap, HttpClient, HttpClientBuilder};
use serde_aux::prelude::{deserialize_default_from_null, deserialize_number_from_string};
use soroban_env_host::xdr::{
    self, AccountEntry, AccountId, DiagnosticEvent, Error as XdrError, LedgerEntryData, LedgerKey,
    LedgerKeyAccount, PublicKey, ReadXdr, TransactionEnvelope, TransactionMeta, TransactionResult,
    Uint256, WriteXdr,
};
use std::{
    collections,
    fmt::Display,
    str::FromStr,
    time::{Duration, Instant},
};
use termcolor::{Color, ColorChoice, StandardStream, WriteColor};
use termcolor_output::colored;
use tokio::time::sleep;

const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid address: {0}")]
    InvalidAddress(#[from] stellar_strkey::DecodeError),
    #[error("invalid response from server")]
    InvalidResponse,
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
    #[error("transaction submission failed: {0}")]
    TransactionSubmissionFailed(String),
    #[error("expected transaction status: {0}")]
    UnexpectedTransactionStatus(String),
    #[error("transaction submission timeout")]
    TransactionSubmissionTimeout,
    #[error("transaction simulation failed: {0}")]
    TransactionSimulationFailed(String),
    #[error("Missing result in successful response")]
    MissingResult,
    #[error("Failed to read Error response from server")]
    MissingError,
    #[error("cursor is not valid")]
    InvalidCursor,
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
pub struct GetLedgerEntryResponse {
    pub xdr: String,
    // TODO: add lastModifiedLedgerSeq and latestLedger
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct Cost {
    #[serde(rename = "cpuInsns")]
    pub cpu_insns: String,
    #[serde(rename = "memBytes")]
    pub mem_bytes: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct SimulateTransactionResult {
    pub footprint: String,
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub auth: Vec<String>,
    #[serde(deserialize_with = "deserialize_default_from_null")]
    pub events: Vec<String>,
    pub xdr: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct SimulateTransactionResponse {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub results: Vec<SimulateTransactionResult>,
    pub cost: Cost,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
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

    pub async fn get_account(&self, address: &str) -> Result<AccountEntry, Error> {
        let key = LedgerKey::Account(LedgerKeyAccount {
            account_id: AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
                stellar_strkey::ed25519::PublicKey::from_string(address)?.0,
            ))),
        });
        let response = self.get_ledger_entry(key).await?;
        if let LedgerEntryData::Account(entry) =
            LedgerEntryData::read_xdr_base64(&mut response.xdr.as_bytes())?
        {
            Ok(entry)
        } else {
            Err(Error::InvalidResponse)
        }
    }

    pub async fn send_transaction(
        &self,
        tx: &TransactionEnvelope,
    ) -> Result<(TransactionResult, Vec<DiagnosticEvent>), Error> {
        let client = self.client()?;
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
            eprintln!("error: {}", error_result_xdr.ok_or(Error::MissingError)?);
            return Err(Error::TransactionSubmissionFailed(String::new()));
        }
        // even if status == "success" we need to query the transaction status in order to get the result

        // Poll the transaction status
        let start = Instant::now();
        loop {
            let response = self.get_transaction(&hash).await?;
            match response.status.as_str() {
                "SUCCESS" => {
                    // TODO: the caller should probably be printing this
                    eprintln!("{}", response.status);
                    let result_xdr_b64 = response.result_xdr.ok_or(Error::MissingResult)?;
                    let result = TransactionResult::from_xdr_base64(result_xdr_b64)?;
                    let events = match response.result_meta_xdr {
                        None => Vec::new(),
                        Some(m) => extract_events(TransactionMeta::from_xdr_base64(m)?),
                    };
                    return Ok((result, events));
                }
                "FAILED" => {
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
        let base64_tx = tx.to_xdr_base64()?;
        let response: SimulateTransactionResponse = self
            .client()?
            .request("simulateTransaction", rpc_params![base64_tx])
            .await?;
        match response.error {
            None => Ok(response),
            Some(e) => Err(Error::TransactionSimulationFailed(e)),
        }
    }

    pub async fn get_transaction(&self, tx_id: &str) -> Result<GetTransactionResponse, Error> {
        Ok(self
            .client()?
            .request("getTransaction", rpc_params![tx_id])
            .await?)
    }

    pub async fn get_ledger_entry(&self, key: LedgerKey) -> Result<GetLedgerEntryResponse, Error> {
        let base64_key = key.to_xdr_base64()?;
        Ok(self
            .client()?
            .request("getLedgerEntry", rpc_params![base64_key])
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

        let mut object = collections::BTreeMap::<&str, jsonrpsee_core::JsonValue>::new();
        match start {
            EventStart::Ledger(l) => object.insert("startLedger", l.to_string().into()),
            EventStart::Cursor(c) => pagination.insert("cursor".to_string(), c.into()),
        };
        object.insert("filters", vec![filters].into());
        object.insert("pagination", pagination.into());

        Ok(self
            .client()?
            .request("getEvents", Some(types::ParamsSer::Map(object)))
            .await?)
    }
}

fn extract_events(tx_meta: TransactionMeta) -> Vec<DiagnosticEvent> {
    match tx_meta {
        TransactionMeta::V3(v3) => {
            // NOTE: we assume there can only be one operation, since we only send one
            if v3.diagnostic_events.len() == 1 {
                v3.diagnostic_events[0].events.clone().into()
            } else if v3.events.len() == 1 {
                v3.events[0]
                    .events
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
