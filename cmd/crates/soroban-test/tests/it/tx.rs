use soroban_cli::xdr::{Limits, ReadXdr, TransactionEnvelope, WriteXdr};
use soroban_test::{AssertExt, TestEnv};

const SOURCE: &str = "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI";

#[tokio::test]
async fn build_simulate_sign_send() {
    let sandbox = &TestEnv::new();
    let tx_base64 = sandbox
        .new_assert_cmd("tx")
        .args(["new", "payment", "--destination", SOURCE, "--amount", "222"])
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.fee, 100);
    // set transaction options set fee
    let new_tx = sandbox
        .new_assert_cmd("tx")
        .arg("set")
        .arg("--fee")
        .arg("10000")
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env_two = TransactionEnvelope::from_xdr_base64(&new_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env_two).unwrap();
    assert_eq!(tx.fee, 10000);
}
