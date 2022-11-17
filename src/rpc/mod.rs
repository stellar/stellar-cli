use jsonrpsee_core::{client::ClientT, rpc_params};
use jsonrpsee_http_client::{HeaderMap, HttpClient, HttpClientBuilder};
use soroban_env_host::xdr::{Error as XdrError, ScVal, TransactionEnvelope, WriteXdr};
use std::time::{Duration, Instant};
use tokio::time::sleep;

const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("jsonrpc error: {0}")]
    JsonRpc(#[from] jsonrpsee_core::Error),
    #[error("transaction submission failed")]
    TransactionSubmissionFailed,
    #[error("expected transaction status: {0}")]
    UnexpectedTransactionStatus(String),
    #[error("transaction submission timeout")]
    TransactionSubmissionTimeout,
    #[error("transaction simulation failed: {0}")]
    TransactionSimulationFailed(String),
}

// TODO: this should also be used by serve
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct GetAccountResponse {
    pub id: String,
    pub sequence: String,
    // TODO: add balances
}

// TODO: this should also be used by serve
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct SendTransactionResponse {
    pub id: String,
    pub status: String,
    // TODO: add error
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct TransactionStatusResult {
    pub xdr: String,
}

// TODO: this should also be used by serve
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct GetTransactionStatusResponse {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub results: Vec<TransactionStatusResult>,
}

// TODO: this should also be used by serve
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct GetContractDataResponse {
    pub xdr: String,
    // TODO: add lastModifiedLedgerSeq and latestLedger
}

// TODO: this should also be used by serve
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct Cost {
    #[serde(rename = "cpuInsns")]
    pub cpu_insns: String,
    #[serde(rename = "memBytes")]
    pub mem_bytes: String,
}
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct SimulateTransactionResponse {
    pub footprint: String,
    pub cost: Cost,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
    // TODO: add results and latestLedger
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct GetEventsResponse {
    pub events: Vec<Event>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct Event {
    pub ledger: String,

    #[serde(rename = "ledgerClosedAt")]
    pub ledger_closed_at: String,

    #[serde(rename = "contractId")]
    pub contract_id: String,

    pub id: String,
    pub paging_token: String,
    pub topic: Vec<String>,
    pub value: String,
}

pub struct Client {
    base_url: String,
}

impl Client {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
        }
    }

    fn client(&self) -> Result<HttpClient, Error> {
        let url = self.base_url.clone();
        let mut headers = HeaderMap::new();
        headers.insert("X-Client-Name", "soroban-cli".parse().unwrap());
        let version = VERSION.unwrap_or("devel");
        headers.insert("X-Client-Version", version.parse().unwrap());
        // TODO: We should consider migrating the server subcommand to jsonrpsee
        Ok(HttpClientBuilder::default()
            .set_headers(headers)
            .build(url)?)
    }

    pub async fn get_account(&self, account_id: &str) -> Result<GetAccountResponse, Error> {
        Ok(self
            .client()?
            .request("getAccount", rpc_params![account_id])
            .await?)
    }

    pub async fn send_transaction(
        &self,
        tx: &TransactionEnvelope,
    ) -> Result<Vec<TransactionStatusResult>, Error> {
        let client = self.client()?;
        let SendTransactionResponse { id, status } = client
            .request("sendTransaction", rpc_params![tx.to_xdr_base64()?])
            .await
            .map_err(|_| Error::TransactionSubmissionFailed)?;

        if status == "error" {
            return Err(Error::TransactionSubmissionFailed);
        }
        // even if status == "success" we need to query the transaction status in order to get the result

        // Poll the transaction status
        let start = Instant::now();
        loop {
            let response = self.get_transaction_status(&id).await?;
            match response.status.as_str() {
                "success" => {
                    // TODO: the caller should probably be printing this
                    eprintln!("{}", response.status);
                    return Ok(response.results);
                }
                "error" => {
                    // TODO: provide a more elaborate error
                    return Err(Error::TransactionSubmissionFailed);
                }
                "pending" => (),
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

    pub async fn get_transaction_status(
        &self,
        tx_id: &str,
    ) -> Result<GetTransactionStatusResponse, Error> {
        Ok(self
            .client()?
            .request("getTransactionStatus", rpc_params![tx_id])
            .await?)
    }

    pub async fn get_contract_data(
        &self,
        contract_id: &str,
        key: ScVal,
    ) -> Result<GetContractDataResponse, Error> {
        let base64_key = key.to_xdr_base64()?;
        Ok(self
            .client()?
            .request("getContractData", rpc_params![contract_id, base64_key])
            .await?)
    }

    pub async fn get_events(
        &self,
        _contract_ids: &Vec<String>,
        _topics: &Vec<String>,
    ) -> Result<GetEventsResponse, Error> {
        // Ok(self
        //     .client()?
        //     .request("getEvents", rpc_params![contract_ids, topics])
        //     .await?)

        Ok(GetEventsResponse {
            events: vec![Event {
                ledger: "43601283".to_string(),
                ledger_closed_at: "2022-11-16T16:10:41Z".to_string(),
                contract_id: "e3e82a76cc316f6289fd1ffbdf315da0f2c6be9582b84b9983a402f02ea0fff7"
                    .to_string(),
                id: "0164090849041387521-0000000003".to_string(),
                paging_token: "164090849041387521-3".to_string(),
                topic: vec![
                    "AAAABQAAAAh0cmFuc2Zlcg==".to_string(), // "transfer" symbol
                    "AAAAAQB6Mcc=".to_string(),             // 8008135 u32
                ],
                value: "AAAABQAAAAh0cmFuc2Zlcg==".to_string(),
            }],
        })
    }
}
