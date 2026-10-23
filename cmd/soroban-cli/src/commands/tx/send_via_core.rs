use std::ffi::OsString;

use serde::{Deserialize, Serialize};
use stellar_xdr::{Limits, WriteXdr};
use url::Url;

use crate::{
    commands::global,
    print::Print,
    utils::{http, XDR_DEPTH_LIMIT},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] stellar_xdr::Error),
    #[error("invalid core URL: {0}")]
    InvalidUrl(String),
    #[error(transparent)]
    HttpClient(#[from] reqwest::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoreTxResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
/// Command to send a transaction envelope directly to a stellar-core HTTP endpoint
/// e.g. `stellar tx send-via-core file.txt --core-url http://localhost:11626` or `cat file.txt | stellar tx send-via-core --core-url http://localhost:11626`
pub struct Cmd {
    /// Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty
    #[arg()]
    pub tx_xdr: Option<OsString>,

    /// Stellar Core HTTP endpoint (e.g. `http://localhost:11626`)
    #[arg(long = "core-url", env = "STELLAR_CORE_URL")]
    pub core_url: String,
}

pub fn build_tx_url(core_url: &str, tx_xdr_base64: &str) -> Result<Url, Error> {
    let mut url = Url::parse(core_url).map_err(|_| Error::InvalidUrl(core_url.to_string()))?;
    let trimmed_path = url.path().trim_end_matches('/');
    let new_path = if trimmed_path.ends_with("/tx") {
        trimmed_path.to_string()
    } else if trimmed_path.is_empty() {
        "/tx".to_string()
    } else {
        format!("{trimmed_path}/tx")
    };
    url.set_path(&new_path);
    url.query_pairs_mut().append_pair("blob", tx_xdr_base64);
    Ok(url)
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let response = self.execute(global_args.quiet).await?;
        println!("{}", serde_json::to_string_pretty(&response)?);
        Ok(())
    }

    pub async fn execute(&self, quiet: bool) -> Result<CoreTxResponse, Error> {
        let tx_env = super::xdr::tx_envelope_from_input(&self.tx_xdr)?;
        let tx_xdr_base64 = tx_env.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?;
        let url = build_tx_url(&self.core_url, &tx_xdr_base64)?;

        let print = Print::new(quiet);
        print.infoln(format!(
            "Submitting transaction to stellar-core ({})",
            self.core_url
        ));

        let response = http::client()
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<CoreTxResponse>()
            .await?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use stellar_xdr::{
        Memo, MuxedAccount, Preconditions, SequenceNumber, Transaction, TransactionEnvelope,
        TransactionExt, TransactionV1Envelope, Uint256, VecM,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn test_build_tx_url_root() {
        let url = build_tx_url("http://localhost:11626", "AAAA+w==").unwrap();
        assert_eq!(
            url.as_str(),
            "http://localhost:11626/tx?blob=AAAA%2Bw%3D%3D"
        );
    }

    #[test]
    fn test_build_tx_url_trailing_slash() {
        let url = build_tx_url("http://localhost:11626/", "AAAA/w==").unwrap();
        assert_eq!(
            url.as_str(),
            "http://localhost:11626/tx?blob=AAAA%2Fw%3D%3D"
        );
    }

    #[test]
    fn test_build_tx_url_already_has_tx_path() {
        let url = build_tx_url("http://localhost:11626/tx", "AAAA==").unwrap();
        assert_eq!(url.as_str(), "http://localhost:11626/tx?blob=AAAA%3D%3D");
    }

    #[test]
    fn test_build_tx_url_nested_prefix() {
        let url = build_tx_url("https://core.example.com/stellar/", "AAAA").unwrap();
        assert_eq!(
            url.as_str(),
            "https://core.example.com/stellar/tx?blob=AAAA"
        );
    }

    #[test]
    fn test_build_tx_url_invalid() {
        let err = build_tx_url("not a valid url", "AAAA").unwrap_err();
        assert!(matches!(err, Error::InvalidUrl(_)));
    }

    #[test]
    fn test_parse_core_tx_response_pending() {
        let raw = r#"{"status": "PENDING"}"#;
        let parsed: CoreTxResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.status, "PENDING");
        assert_eq!(parsed.error, None);
        assert!(parsed.extra.is_empty());
    }

    #[test]
    fn test_parse_core_tx_response_error_and_extra_fields() {
        let raw = r#"{
            "status": "ERROR",
            "error": "AAAAAAAAAGT////7AAAAAA==",
            "diagnostic_events": ["AAA="]
        }"#;
        let parsed: CoreTxResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.status, "ERROR");
        assert_eq!(parsed.error.as_deref(), Some("AAAAAAAAAGT////7AAAAAA=="));
        assert!(parsed.extra.contains_key("diagnostic_events"));
    }

    #[test]
    fn test_cli_parse_args() {
        let cmd = Cmd::try_parse_from([
            "send-via-core",
            "AAAA",
            "--core-url",
            "http://127.0.0.1:11626",
        ])
        .unwrap();
        assert_eq!(cmd.tx_xdr, Some(OsString::from("AAAA")));
        assert_eq!(cmd.core_url, "http://127.0.0.1:11626");
    }

    #[test]
    fn test_cli_parse_stdin_mode() {
        let cmd =
            Cmd::try_parse_from(["send-via-core", "--core-url", "http://127.0.0.1:11626"]).unwrap();
        assert_eq!(cmd.tx_xdr, None);
        assert_eq!(cmd.core_url, "http://127.0.0.1:11626");
    }

    #[tokio::test]
    async fn test_execute_submits_to_core_tx_endpoint() {
        let tx_env = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: Transaction {
                source_account: MuxedAccount::Ed25519(Uint256([0u8; 32])),
                fee: 100,
                seq_num: SequenceNumber(1),
                cond: Preconditions::None,
                memo: Memo::None,
                operations: VecM::default(),
                ext: TransactionExt::V0,
            },
            signatures: VecM::default(),
        });
        let xdr_b64 = tx_env.to_xdr_base64(Limits::none()).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 2048];
            let n = socket.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.starts_with("GET /tx?blob="));

            let body = r#"{"status":"PENDING"}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(resp.as_bytes()).await.unwrap();
        });

        let cmd = Cmd {
            tx_xdr: Some(OsString::from(xdr_b64)),
            core_url: format!("http://{addr}"),
        };

        let result = cmd.execute(true).await.unwrap();
        assert_eq!(result.status, "PENDING");
        assert_eq!(result.error, None);
        server.await.unwrap();
    }
}
