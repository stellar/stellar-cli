//! x402 (v2) payment-challenge wire types and the base64-JSON header codec.
//!
//! These mirror the `@x402/core` / `@x402/stellar` TypeScript reference
//! implementation: a `402 Payment Required` response carries a `PAYMENT-REQUIRED`
//! declaration (as a base64-encoded JSON header, or the same JSON in the body),
//! the client replays the request with a base64-encoded JSON `PAYMENT-SIGNATURE`
//! proof, and a successful response carries a base64-encoded JSON
//! `PAYMENT-RESPONSE` settlement.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::Value;

/// Header carrying the server's payment declaration on a `402` response.
pub const PAYMENT_REQUIRED_HEADER: &str = "PAYMENT-REQUIRED";
/// Header carrying the client's signed payment proof on the replayed request.
pub const PAYMENT_SIGNATURE_HEADER: &str = "PAYMENT-SIGNATURE";
/// Header carrying the facilitator's settlement result on a paid response.
pub const PAYMENT_RESPONSE_HEADER: &str = "PAYMENT-RESPONSE";

/// CAIP-2 network id for Stellar testnet, as used in `accepts[].network`.
const STELLAR_TESTNET_CAIP2: &str = "stellar:testnet";
/// CAIP-2 network id for Stellar mainnet, as used in `accepts[].network`.
const STELLAR_PUBNET_CAIP2: &str = "stellar:pubnet";

const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";
const PUBNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("the server's {header} header is not valid base64-encoded JSON")]
    MalformedHeader { header: &'static str },

    #[error("the server's 402 response carried no payment declaration")]
    MissingChallenge,

    #[error(
        "the server offered no payment option for {network}.\n\
         Offered networks: {offered}"
    )]
    NoMatchingRequirement { network: String, offered: String },

    #[error("the payment option uses an unsupported scheme `{0}`; only `exact` is supported")]
    UnsupportedScheme(String),

    #[error("the payment option is missing an amount")]
    MissingAmount,

    #[error("the payment option amount `{0}` is not a valid integer in the asset's smallest unit")]
    InvalidAmount(String),

    #[error(
        "the payment option does not sponsor fees (`extra.areFeesSponsored` is not true); \
         the exact scheme requires a fee-sponsoring facilitator"
    )]
    FeesNotSponsored,

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::MalformedHeader { .. } => "malformed_header",
            Error::MissingChallenge => "missing_challenge",
            Error::NoMatchingRequirement { .. } => "no_matching_requirement",
            Error::UnsupportedScheme(_) => "unsupported_scheme",
            Error::MissingAmount => "missing_amount",
            Error::InvalidAmount(_) => "invalid_amount",
            Error::FeesNotSponsored => "fees_not_sponsored",
            Error::Serde(_) => "internal",
        }
    }
}

/// The server's `402` payment declaration.
///
/// The `accepts` options and `resource` are kept as raw JSON so the chosen
/// option can be echoed back verbatim in the payment proof's `accepted` field,
/// exactly as the server sent it.
#[derive(Debug, Clone)]
pub struct Challenge {
    pub x402_version: u32,
    pub accepts: Vec<Value>,
    pub resource: Option<Value>,
    /// Why the server rejected a payment, on a `402` returned for a replayed request.
    pub error: Option<String>,
}

/// The typed subset of a payment requirement the client needs to build the
/// on-chain leg. Unknown fields are ignored here; the full option is preserved
/// as the raw [`Value`] alongside it (see [`Challenge::select_stellar`]).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Requirements {
    pub scheme: String,
    pub network: String,
    pub asset: String,
    #[serde(rename = "payTo")]
    pub pay_to: String,
    #[serde(default)]
    pub amount: Option<String>,
    #[serde(rename = "maxAmountRequired", default)]
    pub max_amount_required: Option<String>,
    #[serde(rename = "maxTimeoutSeconds", default)]
    pub max_timeout_seconds: u64,
    #[serde(default)]
    pub extra: Option<Extra>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Extra {
    #[serde(rename = "areFeesSponsored", default)]
    pub are_fees_sponsored: Option<bool>,
}

impl Requirements {
    /// The payment amount in the asset's smallest unit. The spec allows either
    /// `amount` or the legacy `maxAmountRequired`; prefer the former.
    pub fn amount_raw(&self) -> Result<i128, Error> {
        let raw = self
            .amount
            .as_deref()
            .or(self.max_amount_required.as_deref())
            .ok_or(Error::MissingAmount)?;
        raw.parse::<i128>()
            .map_err(|_| Error::InvalidAmount(raw.to_string()))
    }

    /// Whether the facilitator sponsors fees. The exact scheme requires this,
    /// because the client signs only the authorization entry and never the
    /// transaction envelope — the facilitator pays and submits.
    #[must_use]
    pub fn fees_sponsored(&self) -> bool {
        self.extra
            .as_ref()
            .and_then(|e| e.are_fees_sponsored)
            .unwrap_or(false)
    }
}

/// Map a CAIP-2 Stellar network id (`stellar:testnet` / `stellar:pubnet`) to the
/// network passphrase used for signing. Returns `None` for ids this client
/// doesn't recognize.
#[must_use]
pub fn passphrase_for_caip2(network: &str) -> Option<&'static str> {
    match network {
        STELLAR_TESTNET_CAIP2 => Some(TESTNET_PASSPHRASE),
        STELLAR_PUBNET_CAIP2 => Some(PUBNET_PASSPHRASE),
        _ => None,
    }
}

impl Challenge {
    /// Parse the challenge from a `402` response's `PAYMENT-REQUIRED` header
    /// (base64-encoded JSON) when present, otherwise from the JSON body.
    pub fn parse(header: Option<&str>, body: &[u8]) -> Result<Self, Error> {
        let value: Value = if let Some(header) = header.filter(|h| !h.is_empty()) {
            let decoded = BASE64
                .decode(header.trim())
                .map_err(|_| Error::MalformedHeader {
                    header: PAYMENT_REQUIRED_HEADER,
                })?;
            serde_json::from_slice(&decoded).map_err(|_| Error::MalformedHeader {
                header: PAYMENT_REQUIRED_HEADER,
            })?
        } else if !body.is_empty() {
            serde_json::from_slice(body).map_err(|_| Error::MissingChallenge)?
        } else {
            return Err(Error::MissingChallenge);
        };

        Self::from_value(value)
    }

    fn from_value(value: Value) -> Result<Self, Error> {
        let x402_version = value
            .get("x402Version")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(Error::MissingChallenge)?;
        let accepts = value
            .get("accepts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let resource = value.get("resource").cloned();
        Ok(Self {
            x402_version,
            accepts,
            resource,
            error: value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    /// Select the `exact`-scheme option whose network matches `network_passphrase`,
    /// returning both the raw option (to echo in the proof) and its typed view.
    ///
    /// Errors if no option targets the configured network, or the matching option
    /// uses an unsupported scheme or doesn't sponsor fees.
    pub fn select_stellar(&self, network_passphrase: &str) -> Result<(Value, Requirements), Error> {
        let mut offered = Vec::new();
        let mut matched: Option<(Value, Requirements)> = None;

        for raw in &self.accepts {
            let req: Requirements = match serde_json::from_value(raw.clone()) {
                Ok(req) => req,
                Err(_) => continue,
            };
            offered.push(req.network.clone());
            if passphrase_for_caip2(&req.network) == Some(network_passphrase) {
                matched = Some((raw.clone(), req));
                break;
            }
        }

        let (raw, req) = matched.ok_or_else(|| Error::NoMatchingRequirement {
            network: network_passphrase.to_string(),
            offered: if offered.is_empty() {
                "(none)".to_string()
            } else {
                offered.join(", ")
            },
        })?;

        if req.scheme != "exact" {
            return Err(Error::UnsupportedScheme(req.scheme));
        }
        if !req.fees_sponsored() {
            return Err(Error::FeesNotSponsored);
        }

        Ok((raw, req))
    }
}

/// The facilitator's settlement result, decoded from the `PAYMENT-RESPONSE`
/// header on a successful paid response.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Settlement {
    #[serde(default)]
    pub transaction: Option<String>,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(rename = "errorReason", default)]
    pub error_reason: Option<String>,
    #[serde(rename = "errorMessage", default)]
    pub error_message: Option<String>,
}

impl Settlement {
    /// Decode the settlement from a base64-encoded JSON `PAYMENT-RESPONSE` header.
    pub fn parse(header: &str) -> Result<Self, Error> {
        let decoded = BASE64
            .decode(header.trim())
            .map_err(|_| Error::MalformedHeader {
                header: PAYMENT_RESPONSE_HEADER,
            })?;
        serde_json::from_slice(&decoded).map_err(|_| Error::MalformedHeader {
            header: PAYMENT_RESPONSE_HEADER,
        })
    }
}

/// Why the server rejected a replayed payment: the facilitator's settlement
/// failure from `PAYMENT-RESPONSE`, else the verification failure carried in a
/// fresh `PAYMENT-REQUIRED` declaration.
#[must_use]
pub fn rejection_reason(
    payment_response: Option<&str>,
    payment_required: Option<&str>,
) -> Option<String> {
    let settlement = payment_response
        .and_then(|header| Settlement::parse(header).ok())
        .and_then(|s| s.error_message.or(s.error_reason));
    settlement.or_else(|| {
        payment_required
            .filter(|header| !header.is_empty())
            .and_then(|header| Challenge::parse(Some(header), b"").ok())
            .and_then(|c| c.error)
    })
}

/// Encode a payment proof payload as a base64-encoded JSON `PAYMENT-SIGNATURE`
/// header value.
pub fn encode_payment_signature(payload: &Value) -> Result<String, Error> {
    Ok(BASE64.encode(serde_json::to_vec(payload)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_challenge() -> Value {
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
            }],
            "resource": { "url": "https://example.test/weather" }
        })
    }

    #[test]
    fn parses_challenge_from_base64_header() {
        let header = BASE64.encode(serde_json::to_vec(&sample_challenge()).unwrap());
        let challenge = Challenge::parse(Some(&header), b"").unwrap();
        assert_eq!(challenge.x402_version, 2);
        assert_eq!(challenge.accepts.len(), 1);
        assert!(challenge.resource.is_some());
    }

    #[test]
    fn parses_challenge_from_body_when_no_header() {
        let body = serde_json::to_vec(&sample_challenge()).unwrap();
        let challenge = Challenge::parse(None, &body).unwrap();
        assert_eq!(challenge.x402_version, 2);
    }

    #[test]
    fn malformed_header_is_rejected() {
        let err = Challenge::parse(Some("not base64!!"), b"").unwrap_err();
        assert_eq!(err.error_type(), "malformed_header");
    }

    #[test]
    fn selects_matching_network_and_reads_fields() {
        let challenge =
            Challenge::parse(None, &serde_json::to_vec(&sample_challenge()).unwrap()).unwrap();
        let (raw, req) = challenge.select_stellar(TESTNET_PASSPHRASE).unwrap();
        assert_eq!(req.scheme, "exact");
        assert_eq!(req.amount_raw().unwrap(), 10_000_000);
        assert_eq!(req.max_timeout_seconds, 60);
        assert!(req.fees_sponsored());
        // The raw option is preserved verbatim for echoing in the proof.
        assert_eq!(raw["payTo"], req.pay_to);
    }

    #[test]
    fn no_option_for_configured_network_errors() {
        let challenge =
            Challenge::parse(None, &serde_json::to_vec(&sample_challenge()).unwrap()).unwrap();
        let err = challenge.select_stellar(PUBNET_PASSPHRASE).unwrap_err();
        assert_eq!(err.error_type(), "no_matching_requirement");
    }

    #[test]
    fn unsponsored_fees_are_rejected() {
        let mut value = sample_challenge();
        value["accepts"][0]["extra"]["areFeesSponsored"] = json!(false);
        let challenge = Challenge::parse(None, &serde_json::to_vec(&value).unwrap()).unwrap();
        let err = challenge.select_stellar(TESTNET_PASSPHRASE).unwrap_err();
        assert_eq!(err.error_type(), "fees_not_sponsored");
    }

    #[test]
    fn legacy_max_amount_required_is_honored() {
        let mut value = sample_challenge();
        let accept = &mut value["accepts"][0];
        accept["amount"] = Value::Null;
        accept.as_object_mut().unwrap().remove("amount");
        accept["maxAmountRequired"] = json!("42");
        let challenge = Challenge::parse(None, &serde_json::to_vec(&value).unwrap()).unwrap();
        let (_, req) = challenge.select_stellar(TESTNET_PASSPHRASE).unwrap();
        assert_eq!(req.amount_raw().unwrap(), 42);
    }

    #[test]
    fn settlement_round_trips_through_header() {
        let header = BASE64.encode(
            serde_json::to_vec(&json!({ "transaction": "abc123", "network": "stellar:testnet" }))
                .unwrap(),
        );
        let settlement = Settlement::parse(&header).unwrap();
        assert_eq!(settlement.transaction.as_deref(), Some("abc123"));
    }

    fn encode(value: &Value) -> String {
        BASE64.encode(serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn rejection_reason_reads_verification_error_from_payment_required() {
        let mut value = sample_challenge();
        value["error"] = json!("invalid_exact_stellar_payload_malformed");
        assert_eq!(
            rejection_reason(None, Some(&encode(&value))).as_deref(),
            Some("invalid_exact_stellar_payload_malformed")
        );
    }

    #[test]
    fn rejection_reason_prefers_settlement_failure_message() {
        let settlement = json!({
            "success": false,
            "errorReason": "transaction_failed",
            "errorMessage": "transaction failed on-chain",
        });
        let mut challenge = sample_challenge();
        challenge["error"] = json!("other");
        assert_eq!(
            rejection_reason(Some(&encode(&settlement)), Some(&encode(&challenge))).as_deref(),
            Some("transaction failed on-chain")
        );
    }

    #[test]
    fn rejection_reason_falls_back_to_settlement_reason() {
        let settlement = json!({ "success": false, "errorReason": "transaction_failed" });
        assert_eq!(
            rejection_reason(Some(&encode(&settlement)), None).as_deref(),
            Some("transaction_failed")
        );
    }

    #[test]
    fn rejection_reason_is_none_without_headers() {
        assert_eq!(rejection_reason(None, None), None);
        assert_eq!(rejection_reason(Some("not base64!!"), Some("")), None);
    }

    #[test]
    fn payment_signature_encodes_to_decodable_base64_json() {
        let payload = json!({ "x402Version": 2, "payload": { "transaction": "AAAA" } });
        let encoded = encode_payment_signature(&payload).unwrap();
        let decoded: Value = serde_json::from_slice(&BASE64.decode(&encoded).unwrap()).unwrap();
        assert_eq!(decoded, payload);
    }
}
