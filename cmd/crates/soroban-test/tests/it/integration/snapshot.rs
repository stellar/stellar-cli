use assert_fs::prelude::*;
use predicates::prelude::*;
use soroban_ledger_snapshot::LedgerSnapshot;
use soroban_test::{AssertExt, TestEnv};

#[test]
#[allow(clippy::too_many_lines)]
fn snapshot() {
    let sandbox = &TestEnv::new();
    // Create a couple accounts and a couple contracts, which we'll filter on to
    // make sure we only get the account and contract requested.
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("--fund")
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
        .arg("--fund")
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
            .arg("--fund")
            .arg(format!("k{i}"))
            .assert()
            .success();
    }
    // Create the snapshot.
    sandbox
        .new_assert_cmd("snapshot")
        .arg("create")
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

#[test]
#[allow(clippy::too_many_lines)]
fn snapshot_merge() {
    let sandbox = &TestEnv::new();

    // Create accounts and contracts for two separate snapshots
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("--fund")
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
        .arg("--fund")
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
        .arg(format!("--asset=A2:{account_b}"))
        .assert()
        .success()
        .stdout_as_str();

    // Wait 8 ledgers for a checkpoint
    for i in 1..=8 {
        sandbox
            .new_assert_cmd("keys")
            .arg("generate")
            .arg("--fund")
            .arg(format!("k{i}"))
            .assert()
            .success();
    }

    // Create first snapshot with account_a and contract_a
    sandbox
        .new_assert_cmd("snapshot")
        .arg("create")
        .arg("--address")
        .arg(&account_a)
        .arg("--address")
        .arg(&contract_a)
        .arg("--out=snapshot_a.json")
        .assert()
        .success();

    // Create second snapshot with account_b and contract_b
    sandbox
        .new_assert_cmd("snapshot")
        .arg("create")
        .arg("--address")
        .arg(&account_b)
        .arg("--address")
        .arg(&contract_b)
        .arg("--out=snapshot_b.json")
        .assert()
        .success();

    // Merge the two snapshots
    sandbox
        .new_assert_cmd("snapshot")
        .arg("merge")
        .arg("snapshot_a.json")
        .arg("snapshot_b.json")
        .arg("--out=merged.json")
        .assert()
        .success();

    // Verify the merged snapshot contains all accounts and contracts
    sandbox
        .dir()
        .child("merged.json")
        .assert(predicates::str::contains(&account_a))
        .assert(predicates::str::contains(&account_b))
        .assert(predicates::str::contains(&contract_a))
        .assert(predicates::str::contains(&contract_b));

    let snapshot_a_path = sandbox.dir().join("snapshot_a.json");
    let snapshot_b_path = sandbox.dir().join("snapshot_b.json");
    let merged_path = sandbox.dir().join("merged.json");

    let snapshot_a = LedgerSnapshot::read_file(snapshot_a_path).unwrap();
    let snapshot_b = LedgerSnapshot::read_file(snapshot_b_path).unwrap();
    let merged = LedgerSnapshot::read_file(merged_path).unwrap();

    assert_eq!(merged.protocol_version, snapshot_b.protocol_version);
    assert_eq!(merged.sequence_number, snapshot_b.sequence_number);
    assert_eq!(merged.timestamp, snapshot_b.timestamp);
    assert_eq!(merged.network_id, snapshot_b.network_id);

    // Verify that we have more entries in merged than in either individual snapshot
    assert!(merged.ledger_entries.len() > snapshot_a.ledger_entries.len());
    assert!(merged.ledger_entries.len() > snapshot_b.ledger_entries.len());
}

#[test]
fn snapshot_merge_conflict_resolution() {
    let sandbox = &TestEnv::new();
    let identity = "ineffable-serval-3633";

    // Create an account
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("--fund")
        .arg(identity)
        .assert()
        .success();
    let account = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg(identity)
        .assert()
        .success()
        .stdout_as_str();

    // Wait 8 ledgers for a checkpoint
    for i in 1..=8 {
        sandbox
            .new_assert_cmd("keys")
            .arg("generate")
            .arg("--fund")
            .arg(format!("k{i}"))
            .assert()
            .success();
    }

    // Create first snapshot with the account
    sandbox
        .new_assert_cmd("snapshot")
        .arg("create")
        .arg("--address")
        .arg(&account)
        .arg("--out=snapshot_1.json")
        .assert()
        .success();

    // Wait for another checkpoint to get a different ledger sequence
    for i in 9..=16 {
        sandbox
            .new_assert_cmd("keys")
            .arg("generate")
            .arg("--fund")
            .arg(format!("k{i}"))
            .assert()
            .success();
    }

    // Create second snapshot with the same account at a later ledger sequence
    sandbox
        .new_assert_cmd("snapshot")
        .arg("create")
        .arg("--address")
        .arg(&account)
        .arg("--out=snapshot_2.json")
        .assert()
        .success();

    // Merge the snapshots - snapshot_2 should win
    sandbox
        .new_assert_cmd("snapshot")
        .arg("merge")
        .arg("snapshot_1.json")
        .arg("snapshot_2.json")
        .arg("--out=merged_conflict.json")
        .assert()
        .success();

    // Read snapshots and verify the merged one has the same sequence as snapshot_2
    let snapshot_2_path = sandbox.dir().join("snapshot_2.json");
    let merged_path = sandbox.dir().join("merged_conflict.json");

    let snapshot_2 = LedgerSnapshot::read_file(snapshot_2_path).unwrap();
    let merged = LedgerSnapshot::read_file(merged_path).unwrap();

    // The merged snapshot should have metadata from snapshot_2 (last wins)
    assert_eq!(merged.sequence_number, snapshot_2.sequence_number);
    assert!(merged.sequence_number > 0);
}

#[test]
fn snapshot_merge_multiple() {
    let sandbox = &TestEnv::new();

    // Create three accounts
    let mut accounts = Vec::new();
    for name in ["x", "y", "z"] {
        sandbox
            .new_assert_cmd("keys")
            .arg("generate")
            .arg("--fund")
            .arg(name)
            .assert()
            .success();
        let account = sandbox
            .new_assert_cmd("keys")
            .arg("address")
            .arg(name)
            .assert()
            .success()
            .stdout_as_str();
        accounts.push(account.trim().to_string());
    }

    // Wait 8 ledgers for a checkpoint
    for i in 1..=8 {
        sandbox
            .new_assert_cmd("keys")
            .arg("generate")
            .arg("--fund")
            .arg(format!("k{i}"))
            .assert()
            .success();
    }

    // Create three snapshots, one for each account
    for (i, account) in accounts.iter().enumerate() {
        sandbox
            .new_assert_cmd("snapshot")
            .arg("create")
            .arg("--address")
            .arg(account)
            .arg(format!("--out=snapshot_{}.json", i))
            .assert()
            .success();
    }

    // Merge all three snapshots at once
    sandbox
        .new_assert_cmd("snapshot")
        .arg("merge")
        .arg("snapshot_0.json")
        .arg("snapshot_1.json")
        .arg("snapshot_2.json")
        .arg("--out=merged_multiple.json")
        .assert()
        .success();

    // Read the individual snapshots and merged snapshot to verify
    let snapshot_0_path = sandbox.dir().join("snapshot_0.json");
    let snapshot_1_path = sandbox.dir().join("snapshot_1.json");
    let snapshot_2_path = sandbox.dir().join("snapshot_2.json");
    let merged_path = sandbox.dir().join("merged_multiple.json");

    let snapshot_0 = LedgerSnapshot::read_file(snapshot_0_path).unwrap();
    let snapshot_1 = LedgerSnapshot::read_file(snapshot_1_path).unwrap();
    let snapshot_2 = LedgerSnapshot::read_file(snapshot_2_path).unwrap();
    let merged = LedgerSnapshot::read_file(merged_path).unwrap();

    // Verify that metadata comes from the last snapshot (snapshot_2)
    assert_eq!(merged.sequence_number, snapshot_2.sequence_number);
    assert_eq!(merged.network_id, snapshot_2.network_id);

    // Verify that merged has at least as many entries as the largest individual snapshot
    let max_individual = snapshot_0
        .ledger_entries
        .len()
        .max(snapshot_1.ledger_entries.len())
        .max(snapshot_2.ledger_entries.len());
    assert!(merged.ledger_entries.len() >= max_individual);
}
