use jsonrpsee_core::{client::ClientT, rpc_params};
use jsonrpsee_http_client::{HeaderMap, HttpClient, HttpClientBuilder};
use soroban_env_host::xdr::{Error as XdrError, ScVal, TransactionEnvelope, WriteXdr};
use std::{
    thread::sleep,
    time::{Duration, Instant},
};

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
    // TODO: add results
}

// TODO: this should also be used by serve
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct GetTransactionStatusResponse {
    pub id: String,
    pub status: String,
    // TODO: add results
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
    // TODO: add results and latestLedger
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
        let url = self.base_url.clone() + "/api/v1/jsonrpc";
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
    ) -> Result<SendTransactionResponse, Error> {
        let client = self.client()?;
        let SendTransactionResponse { id, status } = client
            .request("sendTransaction", rpc_params![tx.to_xdr_base64()?])
            .await
            .map_err(|_| Error::TransactionSubmissionFailed)?;

        if status == "success" {
            return Ok(SendTransactionResponse { id, status });
        } else if status == "error" {
            return Err(Error::TransactionSubmissionFailed);
        }

        // Poll the transaction status
        let start = Instant::now();
        loop {
            let response = self.get_transaction_status(&id).await?;
            match response.status.as_str() {
                "success" => {
                    println!("{}", response.status);
                    return Ok(SendTransactionResponse {
                        id: response.id,
                        status: response.status,
                        // TODO: add results
                    });
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
            sleep(Duration::from_secs(1));
        }
    }

    pub async fn simulate_transaction(
        &self,
        tx: &TransactionEnvelope,
    ) -> Result<SimulateTransactionResponse, Error> {
        let base64_tx = tx.to_xdr_base64()?;
        Ok(self
            .client()?
            .request("simulateTransaction", rpc_params![base64_tx])
            .await?)
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
}
