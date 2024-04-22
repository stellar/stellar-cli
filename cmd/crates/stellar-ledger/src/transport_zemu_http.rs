// I was getting an error when trying to install this as a crate, so i'm just going to take the pieces that i need an put them here since i am not using the gRpc transport right now anyway
// error: failed to run custom build command for `ledger-transport-zemu v0.10.0 (/Users/elizabethengelman/Projects/Aha-Labs/ledger-rs/ledger-transport-zemu)`

// Caused by:
//   process didn't exit successfully: `/Users/elizabethengelman/Projects/Aha-Labs/ledger-rs/target/debug/build/ledger-transport-zemu-e14fd4e52eee79e2/build-script-build` (exit status: 101)
//   --- stdout
//   cargo:rerun-if-changed=zemu.proto

//   --- stderr
//   thread 'main' panicked at /Users/elizabethengelman/.cargo/registry/src/index.crates.io-6f17d22bba15001f/protoc-2.18.2/src/lib.rs:203:17:
//   protoc binary not found: cannot find binary path
//   note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
// warning: build failed, waiting for other jobs to finish...

// this if from: https://github.com/Zondax/ledger-rs/blob/master/ledger-transport-zemu/src/lib.rs
// removed the grpc stuff
/*******************************************************************************
*   (c) 2022 Zondax AG
*
*  Licensed under the Apache License, Version 2.0 (the "License");
*  you may not use this file except in compliance with the License.
*  You may obtain a copy of the License at
*
*      http://www.apache.org/licenses/LICENSE-2.0
*
*  Unless required by applicable law or agreed to in writing, software
*  distributed under the License is distributed on an "AS IS" BASIS,
*  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
*  See the License for the specific language governing permissions and
*  limitations under the License.
********************************************************************************/

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::{Client as HttpClient, Response};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::time::Duration;

use ledger_transport::{async_trait, APDUAnswer, APDUCommand, Exchange};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerZemuError {
    /// zemu reponse error
    #[error("Zemu response error")]
    ResponseError,
    /// Inner error
    #[error("Ledger inner error")]
    InnerError,
}

pub struct TransportZemuHttp {
    url: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ZemuRequest {
    apdu_hex: String,
}

#[derive(Deserialize, Debug, Clone)]
struct ZemuResponse {
    data: String,
    error: Option<String>,
}

impl TransportZemuHttp {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            url: format!("http://{host}:{port}"),
        }
    }
}

#[async_trait]
impl Exchange for TransportZemuHttp {
    type Error = LedgerZemuError;
    type AnswerType = Vec<u8>;

    async fn exchange<I>(
        &self,
        command: &APDUCommand<I>,
    ) -> Result<APDUAnswer<Self::AnswerType>, Self::Error>
    where
        I: Deref<Target = [u8]> + Send + Sync,
    {
        let raw_command = hex::encode(command.serialize());
        let request = ZemuRequest {
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
                tracing::error!("create http client error: {:?}", e);
                LedgerZemuError::InnerError
            })?;
        tracing::debug!("http response: {:?}", resp);

        if resp.status().is_success() {
            let result: ZemuResponse = resp.json().await.map_err(|e| {
                tracing::error!("error response: {:?}", e);
                LedgerZemuError::ResponseError
            })?;
            if result.error.is_none() {
                APDUAnswer::from_answer(hex::decode(result.data).expect("decode error"))
                    .map_err(|_| LedgerZemuError::ResponseError)
            } else {
                Err(LedgerZemuError::ResponseError)
            }
        } else {
            tracing::error!("error response: {:?}", resp.status());
            Err(LedgerZemuError::ResponseError)
        }
    }
}
