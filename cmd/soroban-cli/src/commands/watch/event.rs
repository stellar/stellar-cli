use chrono::{DateTime, Utc};

use crate::commands::watch::decode::truncate_addr;

#[derive(Debug, Clone)]
pub struct DecodedValue {
    pub display: String,
}

#[derive(Debug, Clone)]
pub struct EventData {
    pub event_id: String,
    pub contract_id: String,
    pub tx_hash: String,
    pub ledger: u32,
    pub event_type: String,
    pub topics: Vec<DecodedValue>,
    pub value: DecodedValue,
    /// Raw base64 XDR topics, used for spec-based decoding.
    pub raw_topics: Vec<String>,
    /// Raw base64 XDR value, used for spec-based decoding.
    pub raw_value: String,
}

#[derive(Debug, Clone)]
pub struct TransactionData {
    pub tx_hash: String,
    pub ledger: u32,
    pub status: String,
    pub source_account: String,
    pub fee_charged: i64,
    pub operation_count: u32,
    pub operation_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum EventKind {
    Event(EventData),
    Transaction(TransactionData),
}

impl EventKind {
    pub fn ledger(&self) -> u32 {
        match self {
            EventKind::Event(d) => d.ledger,
            EventKind::Transaction(d) => d.ledger,
        }
    }

    pub fn type_label(&self) -> &str {
        match self {
            EventKind::Event(d) => {
                if d.topics.is_empty() {
                    &d.event_type
                } else {
                    &d.topics[0].display
                }
            }
            EventKind::Transaction(_) => "transaction",
        }
    }

    pub fn summary(&self) -> String {
        match self {
            EventKind::Event(d) => {
                let topic = d.topics.first().map_or("?", |t| t.display.as_str());
                format!("{} @ {}", topic, truncate_addr(&d.contract_id))
            }
            EventKind::Transaction(d) => {
                let ops = d.operation_types.join(", ");
                format!(
                    "{} {} op(s): {}",
                    d.status,
                    d.operation_count,
                    if ops.is_empty() {
                        "unknown".to_string()
                    } else {
                        ops
                    }
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppEvent {
    pub id: u64,
    pub timestamp: DateTime<Utc>,
    pub kind: EventKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Error(String),
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionStatus::Connecting => write!(f, "Connecting…"),
            ConnectionStatus::Connected => write!(f, "Connected"),
            ConnectionStatus::Error(e) => write!(f, "Error: {e}"),
        }
    }
}

#[derive(Debug)]
pub enum WorkerMessage {
    NewEvent(AppEvent),
    RpcStatus(ConnectionStatus),
    OlderFetched {
        events: Vec<AppEvent>,
        oldest_available: u32,
    },
    SpecFetched {
        contract_id: String,
        /// Raw spec entries; converted to `Spec` when stored in the app cache.
        spec_entries: Option<Vec<crate::xdr::ScSpecEntry>>,
    },
}
