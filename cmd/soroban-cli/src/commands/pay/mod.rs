//! `stellar checkout <url>` — the x402 / MPP payment client.
//!
//! Fetches an x402-protected resource: on a `402 Payment Required` challenge it
//! builds and authorization-signs a SEP-41 token transfer through the
//! `contract invoke` pipeline (so the payment is previewable via simulation),
//! replays the signed proof to fetch the paid resource, and returns both the
//! settlement receipt and the service response. The client signs only the
//! authorization entry; the facilitator re-sources the transaction, signs the
//! envelope, pays the fees, and submits.

pub mod cap;
pub mod challenge;
pub mod http;
pub mod payment;

use clap::Parser;
use serde_json::Value;

use crate::{
    commands::{global, token::args::OutputFormat},
    config::{self, locator, network, sign_with, UnresolvedMuxedAccount},
    output::Output,
};

use cap::MaxAmount;
use challenge::{Challenge, Settlement};

/// HTTP status returned by an x402 server to demand payment.
const PAYMENT_REQUIRED_STATUS: u16 = 402;

#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The x402-protected resource to fetch and pay for.
    pub url: String,

    /// Account that pays for the resource. Signs and authorizes the on-chain
    /// payment, so it must be an identity or key you control.
    #[arg(
        long = "source-account",
        visible_alias = "source",
        env = "STELLAR_ACCOUNT",
        hide_env_values = true
    )]
    pub source_account: UnresolvedMuxedAccount,

    /// Maximum amount to pay, in the asset's display units (e.g. `1` = 1.0 USDC,
    /// roughly $1). Aborts before signing if the challenge asks for more. Set to
    /// `none` to disable the cap.
    #[arg(
        long,
        env = "STELLAR_CHECKOUT_MAX_AMOUNT",
        default_value = "1",
        value_parser = parse_max_amount
    )]
    pub max_amount: MaxAmount,

    /// Build and authorization-sign the payment and print the proof, without
    /// fetching the paid resource.
    #[arg(long)]
    pub build_only: bool,

    /// Format of the output.
    #[arg(long, default_value = "text")]
    pub output: OutputFormat,

    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    #[command(flatten)]
    pub sign_with: sign_with::Args,
}

fn parse_max_amount(value: &str) -> Result<MaxAmount, cap::Error> {
    value.parse()
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Http(#[from] http::Error),

    #[error(transparent)]
    Challenge(#[from] challenge::Error),

    #[error(transparent)]
    Cap(#[from] cap::Error),

    #[error(transparent)]
    Payment(#[from] payment::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error("the server returned status {status} for `{url}`:\n{body}")]
    UnexpectedStatus {
        status: u16,
        url: String,
        body: String,
    },

    #[error("the payment was rejected (status {status}):\n{body}")]
    PaymentRejected { status: u16, body: String },
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::Config(_) => "config",
            Error::Network(_) => "network",
            Error::Http(e) => e.error_type(),
            Error::Challenge(e) => e.error_type(),
            Error::Cap(e) => e.error_type(),
            Error::Payment(e) => e.error_type(),
            Error::Serde(_) => "internal",
            Error::UnexpectedStatus { .. } => "unexpected_status",
            Error::PaymentRejected { .. } => "payment_rejected",
        }
    }
}

/// The machine-readable receipt of a checkout.
#[derive(Debug, serde::Serialize)]
struct Receipt {
    /// Hash of the settled transaction, as reported by the facilitator. `None`
    /// when the facilitator didn't report one.
    tx_hash: Option<String>,
    /// Block-explorer link for `tx_hash`, when the network is a known public one.
    #[serde(skip_serializing_if = "Option::is_none")]
    explorer_url: Option<String>,
    /// The CAIP-2 network the payment settled on (e.g. `stellar:testnet`).
    network: String,
    /// The paid resource that was fetched after settlement.
    resource: ResourceResponse,
}

/// The service's response to the paid (replayed) request.
#[derive(Debug, serde::Serialize)]
struct ResourceResponse {
    status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<String>,
    /// The body, parsed as JSON when possible, otherwise the raw text.
    body: Value,
}

/// The proof emitted by `--build-only`: the encoded header plus the decoded
/// payload it carries.
#[derive(Debug, serde::Serialize)]
struct Proof {
    payment_signature: String,
    payload: Value,
}

impl Cmd {
    fn config(&self) -> config::Args {
        config::Args {
            network: self.network.clone(),
            source_account: self.source_account.clone(),
            locator: self.locator.clone(),
            sign_with: self.sign_with.clone(),
            fee: None,
            inclusion_fee: None,
        }
    }

    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let output = Output::new(self.output.into(), global_args.quiet);
        // In JSON mode the underlying invoke pipeline's human-readable status
        // logging (stderr) would still fire; run it quietly so machine consumers
        // get clean output without needing `--quiet`.
        let quiet = global_args.quiet || output.is_json();
        let no_cache = global_args.no_cache;
        let config = self.config();
        let network = config.get_network()?;

        // 1. Initial request. A non-402 success means the resource is free.
        let initial = http::get(&self.url, None).await?;
        if initial.status != PAYMENT_REQUIRED_STATUS {
            return self.handle_unpaid(&output, initial);
        }

        output.readable(|print| print.infoln("Received 402 Payment Required; building payment…"));

        // 2. Parse the challenge and select the option for our network.
        let challenge = Challenge::parse(
            initial
                .header(challenge::PAYMENT_REQUIRED_HEADER)
                .as_deref(),
            &initial.body,
        )?;
        let (accepted, req) = challenge.select_stellar(&network.network_passphrase)?;
        let asset = payment::parse_asset(&req.asset)?;
        let amount = req.amount_raw()?;

        // 3. Enforce the spend cap (default 1.0 of the asset ≈ $1) before signing.
        if let MaxAmount::Limit(_) = self.max_amount {
            let decimals = payment::read_decimals(&config, asset, quiet, no_cache).await?;
            self.max_amount.enforce(amount, decimals)?;
        }

        // 4. Build + simulate + authorization-sign the transfer into a proof.
        let payload = payment::build_proof(
            &config,
            asset,
            &req.pay_to,
            amount,
            req.max_timeout_seconds,
            challenge.x402_version,
            &accepted,
            challenge.resource.as_ref(),
            quiet,
            no_cache,
        )
        .await?;
        let payment_signature = challenge::encode_payment_signature(&payload)?;

        // 5. `--build-only`: emit the proof and stop before paying.
        if self.build_only {
            return emit_proof(&output, payment_signature, payload);
        }

        // 6. Replay the request with the proof to settle and fetch the resource.
        output.readable(|print| print.globeln("Submitting payment…"));
        let paid = http::get(&self.url, Some(&payment_signature)).await?;
        if !(200..300).contains(&paid.status) {
            return Err(Error::PaymentRejected {
                status: paid.status,
                body: paid.body_string(),
            });
        }

        // 7. Decode the settlement and build the receipt.
        let settlement = paid
            .header(challenge::PAYMENT_RESPONSE_HEADER)
            .map(|header| Settlement::parse(&header))
            .transpose()?;
        let tx_hash = settlement.and_then(|s| s.transaction);
        let explorer_url = tx_hash
            .as_deref()
            .and_then(|hash| crate::utils::explorer_url_for_transaction(&network, hash));

        let receipt = Receipt {
            tx_hash,
            explorer_url,
            network: req.network,
            resource: resource_response(&paid),
        };

        output.readable(|print| {
            if let Some(hash) = &receipt.tx_hash {
                print.checkln(format!("Payment settled: {hash}"));
            }
            if let Some(url) = &receipt.explorer_url {
                print.linkln(url);
            }
        });

        // Human-readable output writes the fetched resource body to stdout so it
        // stays pipeable; JSON mode writes the full receipt.
        if !output.is_json() {
            println!("{}", paid.body_string());
        }
        output.json_value(&receipt)?;

        Ok(())
    }

    /// Handle a response that didn't demand payment: a success is the free
    /// resource; anything else is an error.
    fn handle_unpaid(&self, output: &Output, response: http::Response) -> Result<(), Error> {
        if !(200..300).contains(&response.status) {
            return Err(Error::UnexpectedStatus {
                status: response.status,
                url: self.url.clone(),
                body: response.body_string(),
            });
        }
        output.readable(|print| print.infoln("No payment required."));
        if !output.is_json() {
            println!("{}", response.body_string());
        }
        output.json_value(&resource_response(&response))?;
        Ok(())
    }
}

/// Emit the `--build-only` proof: the header value on stdout (text mode) or the
/// full proof object (JSON mode).
fn emit_proof(output: &Output, payment_signature: String, payload: Value) -> Result<(), Error> {
    output.readable(|print| {
        print.infoln("Built payment proof (not submitted):");
    });
    if !output.is_json() {
        println!("{payment_signature}");
    }
    output.json_value(&Proof {
        payment_signature,
        payload,
    })?;
    Ok(())
}

/// Decode an HTTP response body into the receipt's resource field, preferring a
/// parsed JSON value and falling back to the raw text.
fn resource_response(response: &http::Response) -> ResourceResponse {
    let body = serde_json::from_slice::<Value>(&response.body)
        .unwrap_or_else(|_| Value::String(response.body_string()));
    ResourceResponse {
        status: response.status,
        content_type: response.content_type(),
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";
    const PUBNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";

    fn cmd(url: String, network_passphrase: &str, max_amount: MaxAmount) -> Cmd {
        Cmd {
            url,
            source_account: UnresolvedMuxedAccount::default(),
            max_amount,
            build_only: false,
            output: OutputFormat::Text,
            network: network::Args {
                // A reachable RPC URL isn't needed: these tests exercise the
                // HTTP challenge loop only, which errors before the on-chain leg.
                rpc_url: Some("http://127.0.0.1:1".to_string()),
                rpc_headers: Vec::new(),
                network_passphrase: Some(network_passphrase.to_string()),
                network: None,
            },
            locator: locator::Args::default(),
            sign_with: sign_with::Args::default(),
        }
    }

    fn global() -> global::Args {
        global::Args {
            locator: locator::Args::default(),
            filter_logs: Vec::new(),
            quiet: true,
            verbose: false,
            very_verbose: false,
            no_cache: true,
        }
    }

    fn testnet_challenge() -> Value {
        json!({
            "x402Version": 2,
            "accepts": [{
                "scheme": "exact",
                "network": "stellar:testnet",
                "asset": "CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA",
                "payTo": "GAREAZZQWHOCBJS236KIE3AWYBVFLSBK7E5UW3ICI3TCRWQKT5LNLCEZ",
                "amount": "10000000",
                "maxTimeoutSeconds": 60,
                "extra": { "areFeesSponsored": true }
            }]
        })
    }

    #[tokio::test]
    async fn free_resource_is_returned_without_payment() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/free")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"weather":"sunny"}"#)
            .create_async()
            .await;

        let cmd = cmd(
            format!("{}/free", server.url()),
            TESTNET_PASSPHRASE,
            MaxAmount::Unlimited,
        );
        assert!(cmd.run(&global()).await.is_ok());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn non_payment_error_status_surfaces_unexpected_status() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/boom")
            .with_status(500)
            .with_body("internal error")
            .create_async()
            .await;

        let cmd = cmd(
            format!("{}/boom", server.url()),
            TESTNET_PASSPHRASE,
            MaxAmount::Unlimited,
        );
        let err = cmd.run(&global()).await.unwrap_err();
        assert_eq!(err.error_type(), "unexpected_status");
    }

    #[tokio::test]
    async fn challenge_for_a_different_network_is_rejected() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/pay")
            .with_status(402)
            .with_body(serde_json::to_string(&testnet_challenge()).unwrap())
            .create_async()
            .await;

        // The challenge only offers testnet, but we're configured for pubnet.
        let cmd = cmd(
            format!("{}/pay", server.url()),
            PUBNET_PASSPHRASE,
            MaxAmount::Unlimited,
        );
        let err = cmd.run(&global()).await.unwrap_err();
        assert_eq!(err.error_type(), "no_matching_requirement");
    }

    #[tokio::test]
    async fn malformed_challenge_is_rejected() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/pay")
            .with_status(402)
            .with_body("this is not json")
            .create_async()
            .await;

        let cmd = cmd(
            format!("{}/pay", server.url()),
            TESTNET_PASSPHRASE,
            MaxAmount::Unlimited,
        );
        let err = cmd.run(&global()).await.unwrap_err();
        assert_eq!(err.error_type(), "missing_challenge");
    }
}
