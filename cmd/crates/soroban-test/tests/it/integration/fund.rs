use predicates::boolean::PredicateBooleanExt;
use soroban_cli::{
    commands::{
        contract::{self, fetch},
        txn_result::TxnResult,
    },
    config::{locator, secret},
};
use soroban_rpc::GetLatestLedgerResponse;
use soroban_test::{AssertExt, TestEnv, LOCAL_NETWORK_PASSPHRASE};

use crate::integration::util::extend_contract;

#[allow(clippy::too_many_lines)]
async fn fund() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test")
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("fund")
        .arg("test")
        .assert()
        .failed()
        .stderr(predicates::str::contains("failed"));
}
