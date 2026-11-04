use httpmock::prelude::*;
use serde_json::{json, Value};
use soroban_cli::xdr::{
    HostFunction, Int128Parts, Limits, OperationBody, Preconditions, ReadXdr, ScVal,
    SorobanCredentials, TransactionEnvelope,
};
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{deploy_sac, sac_id},
    util::{new_account, test_address},
};

/// Base64 JSON `PAYMENT-RESPONSE` for a settled payment with tx hash `abc123`.
const SETTLEMENT_HEADER: &str =
    "eyJzdWNjZXNzIjp0cnVlLCJ0cmFuc2FjdGlvbiI6ImFiYzEyMyIsIm5ldHdvcmsiOiJzdGVsbGFyOmxvY2FsIn0=";

/// An x402 challenge asking for `amount` stroops of `asset`, paid to `pay_to`.
fn challenge(asset: &str, pay_to: &str, amount: i128) -> Value {
    json!({
        "x402Version": 2,
        "accepts": [{
            "scheme": "exact",
            "network": "stellar:local",
            "asset": asset,
            "payTo": pay_to,
            "amount": amount.to_string(),
            "maxTimeoutSeconds": 60,
            "extra": { "areFeesSponsored": true }
        }],
        "resource": { "url": "/pay" }
    })
}

/// Whether the request carries an x402 payment proof.
fn has_payment_signature(req: &HttpMockRequest) -> bool {
    req.headers.as_ref().is_some_and(|headers| {
        headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("payment-signature"))
    })
}

/// Mock the first, unpaid request with a `402` carrying `challenge`.
fn mock_challenge<'a>(server: &'a MockServer, challenge: &Value) -> httpmock::Mock<'a> {
    server.mock(|when, then| {
        when.method(GET)
            .path("/pay")
            .matches(|req| !has_payment_signature(req));
        then.status(402).json_body(challenge.clone());
    })
}

/// Mock the paid retry: any request carrying a payment proof gets `body`.
fn mock_paid<'a>(server: &'a MockServer, body: &[u8]) -> httpmock::Mock<'a> {
    server.mock(|when, then| {
        when.method(GET).path("/pay").matches(has_payment_signature);
        then.status(200)
            .header("content-type", "application/octet-stream")
            .header("payment-response", SETTLEMENT_HEADER)
            .body(body);
    })
}

/// Deploy the native asset contract and create the pay-to account, returning
/// `(asset contract id, pay-to address)`.
fn setup(sandbox: &TestEnv) -> (String, String) {
    deploy_sac(sandbox, "native", "test");
    (sac_id(sandbox, "native"), new_account(sandbox, "merchant"))
}

#[tokio::test]
async fn build_only_emits_a_proof_authorizing_only_the_transfer() {
    let sandbox = &TestEnv::new();
    let (asset, pay_to) = setup(sandbox);
    let payer = test_address(sandbox);
    let amount: i128 = 1_000_000;

    let server = MockServer::start();
    let challenge_mock = mock_challenge(&server, &challenge(&asset, &pay_to, amount));
    let paid_mock = mock_paid(&server, b"paid");

    let stdout = sandbox
        .new_assert_cmd("checkout")
        .args([&server.url("/pay"), "--build-only", "--output", "json"])
        .assert()
        .success()
        .stdout_as_str();
    challenge_mock.assert();
    paid_mock.assert_hits(0);

    let proof: Value = serde_json::from_str(&stdout).unwrap();
    assert!(proof["payment_signature"].as_str().is_some());
    assert_eq!(proof["payload"]["accepted"]["payTo"], json!(pay_to));

    let transaction = proof["payload"]["payload"]["transaction"].as_str().unwrap();
    let TransactionEnvelope::Tx(envelope) =
        TransactionEnvelope::from_xdr_base64(transaction, Limits::none()).unwrap()
    else {
        panic!("expected a v1 transaction envelope");
    };
    assert!(envelope.signatures.is_empty());
    let Preconditions::Time(time_bounds) = &envelope.tx.cond else {
        panic!("expected time bounds, got {:?}", envelope.tx.cond);
    };
    assert!(time_bounds.max_time.0 > 0);

    let [op] = envelope.tx.operations.as_slice() else {
        panic!("expected a single operation");
    };
    let OperationBody::InvokeHostFunction(op) = &op.body else {
        panic!("expected an invoke host function operation");
    };
    let HostFunction::InvokeContract(call) = &op.host_function else {
        panic!("expected a contract call");
    };
    assert_eq!(call.contract_address.to_string(), asset);
    assert_eq!(call.function_name.to_string(), "transfer");
    let [ScVal::Address(from), ScVal::Address(to), value] = call.args.as_slice() else {
        panic!("expected transfer(from, to, amount), got {:?}", call.args);
    };
    assert_eq!(from.to_string(), payer);
    assert_eq!(to.to_string(), pay_to);
    assert_eq!(
        *value,
        ScVal::I128(Int128Parts {
            hi: 0,
            lo: u64::try_from(amount).unwrap()
        })
    );

    let [auth] = op.auth.as_slice() else {
        panic!("expected a single authorization entry");
    };
    let SorobanCredentials::Address(credentials) = &auth.credentials else {
        panic!("expected address credentials");
    };
    assert_ne!(credentials.signature, ScVal::Void);
    assert!(auth.root_invocation.sub_invocations.is_empty());
}

#[tokio::test]
async fn payment_over_the_cap_is_refused_before_signing() {
    let sandbox = &TestEnv::new();
    let (asset, pay_to) = setup(sandbox);

    let server = MockServer::start();
    let challenge_mock = mock_challenge(&server, &challenge(&asset, &pay_to, 20_000_000));
    let paid_mock = mock_paid(&server, b"paid");

    let stdout = sandbox
        .new_assert_cmd("checkout")
        .args([&server.url("/pay"), "--output", "json"])
        .assert()
        .failure()
        .stdout_as_str();
    challenge_mock.assert();
    paid_mock.assert_hits(0);

    let error: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(error["error"]["type"], json!("max_amount_exceeded"));
}

#[tokio::test]
async fn paid_retry_returns_the_resource_and_receipt() {
    let sandbox = &TestEnv::new();
    let (asset, pay_to) = setup(sandbox);
    let body = [0x89, b'P', b'N', b'G', 0xff, 0x00, b'\n', 0xfe];

    let server = MockServer::start();
    let challenge_mock = mock_challenge(&server, &challenge(&asset, &pay_to, 1_000_000));
    let paid_mock = mock_paid(&server, &body);

    let output = sandbox
        .new_assert_cmd("checkout")
        .arg(server.url("/pay"))
        .assert()
        .success();
    assert_eq!(output.get_output().stdout, body);

    let stdout = sandbox
        .new_assert_cmd("checkout")
        .args([&server.url("/pay"), "--output", "json"])
        .assert()
        .success()
        .stdout_as_str();
    challenge_mock.assert_hits(2);
    paid_mock.assert_hits(2);

    let receipt: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(receipt["tx_hash"], json!("abc123"));
    assert_eq!(receipt["network"], json!("stellar:local"));
    assert_eq!(receipt["resource"]["status"], json!(200));
    assert_eq!(
        receipt["resource"]["content_type"],
        json!("application/octet-stream")
    );
}
