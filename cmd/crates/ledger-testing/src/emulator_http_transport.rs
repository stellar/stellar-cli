use ledger_transport::{async_trait, APDUAnswer, APDUCommand, Exchange};
use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE},
    Client as HttpClient, Response,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use hex::{encode, decode};

use std::ops::Deref;
use std::time::Duration;

#[derive(Error, Debug)]
pub enum Error {
    //TODO include the transparent error
    #[error("Apdu response error")]
    ApduResponseError,

    #[error("Ledger inner error")]
    InnerError,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ApduRequest {
    apdu_hex: String,
}

#[derive(Deserialize, Debug, Clone)]
struct ApduResponse {
    data: String,
    error: Option<String>,
}

pub struct EmulatorHttpTransport {
    url: String,
}

impl EmulatorHttpTransport {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            url: format!("http://{host}:{port}"),
        }
    }
}

#[async_trait]
impl Exchange for EmulatorHttpTransport {
    type Error = Error;
    type AnswerType = Vec<u8>;
    async fn exchange<I>(
        &self,
        command: &APDUCommand<I>,
    ) -> Result<APDUAnswer<Self::AnswerType>, Self::Error>
    where
        I: Deref<Target = [u8]> + Send + Sync,
    {
        let raw_command = encode(command.serialize());
        let request = ApduRequest {
            apdu_hex: raw_command,
        };

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let resp: Response = HttpClient::new()
            .post(&self.url)
            .headers(headers)
            .timeout(Duration::from_secs(20))
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                println!("create http client error: {:?}", e);
                Error::InnerError
            })?;
        println!("http response: {:?}", resp);

        if resp.status().is_success() {
            let result: ApduResponse = resp.json().await.map_err(|e| {
                println!("error response: {:?}", e);
                Error::ApduResponseError
            })?;
            if result.error.is_none() {
                APDUAnswer::from_answer(decode(result.data).expect("decode error"))
                    .map_err(|_| Error::ApduResponseError)
            } else {
                Err(Error::ApduResponseError)
            }
        } else {
            println!("error response: {:?}", resp.status());
            Err(Error::ApduResponseError)
        }
    }
}
