use soroban_cli::commands::tx;
use soroban_sdk::xdr::{Limits, ReadXdr, TransactionEnvelope, TransactionV1Envelope, WriteXdr};
use soroban_test::{AssertExt, TestEnv};

use crate::integration::util::{deploy_contract, HELLO_WORLD};

#[tokio::test]
async fn txn_simulate() {
    let sandbox = &TestEnv::new();
    let xdr_base64 = deploy_contract(sandbox, HELLO_WORLD, true).await;
    println!("{xdr_base64}");
    let cmd = tx::simulate::Cmd::default();
    let tx_env = TransactionEnvelope::from_xdr_base64(&xdr_base64, Limits::none()).unwrap();
    let TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) = &tx_env else {
        panic!("Only transaction v1 is supported")
    };
    let assembled = cmd.simulate(tx, &sandbox.client()).await.unwrap();
    let assembled_str = sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .write_stdin(xdr_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    println!("{assembled_str}");
    assert_eq!(
        assembled
            .transaction()
            .to_xdr_base64(Limits::none())
            .unwrap(),
        assembled_str
    );
}
