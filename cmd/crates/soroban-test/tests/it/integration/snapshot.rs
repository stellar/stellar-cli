use assert_fs::prelude::*;
use predicates::prelude::*;
use soroban_test::{AssertExt, TestEnv};

#[test]
#[allow(clippy::too_many_lines)]
fn snapshot() {
    let sandbox = &TestEnv::default();
    // Create a couple accounts and a couple contracts, which we'll filter on to
    // make sure we only get the account and contract requested.
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("a")
        .assert()
        .success();
    let account_a = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("a")
        .assert()
        .success()
        .stdout_as_str();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("b")
        .assert()
        .success();
    let account_b = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("b")
        .assert()
        .success()
        .stdout_as_str();
    let contract_a = sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg(format!("--asset=A1:{account_a}"))
        .assert()
        .success()
        .stdout_as_str();
    let contract_b = sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg(format!("--asset=A2:{account_a}"))
        .assert()
        .success()
        .stdout_as_str();
    // Wait 8 ledgers for a checkpoint by submitting one tx per ledger, in this
    // case a funding transaction.
    for i in 1..=8 {
        sandbox
            .new_assert_cmd("keys")
            .arg("generate")
            .arg(format!("k{i}"))
            .assert()
            .success();
    }
    // Create the snapshot.
    sandbox
        .new_assert_cmd("snapshot")
        .arg("create")
        .arg("--output=json")
        .arg("--address")
        .arg(&account_a)
        .arg("--address")
        .arg(&contract_b)
        .assert()
        .success();
    // Assert that the snapshot includes account a and contract b, but not
    // account b and contract a.
    sandbox
        .dir()
        .child("snapshot.json")
        .assert(predicates::str::contains(&account_a))
        .assert(predicates::str::contains(&account_b).not())
        .assert(predicates::str::contains(&contract_b))
        .assert(predicates::str::contains(&contract_a).not());
}
