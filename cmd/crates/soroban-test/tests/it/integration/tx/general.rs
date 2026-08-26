use soroban_cli::assembled::simulate_and_assemble_transaction;
use soroban_cli::xdr::{
    Limits, OperationBody, ReadXdr, SorobanTransactionData, TransactionEnvelope, TransactionExt,
    WriteXdr,
};
use soroban_test::{AssertExt, TestEnv};

use crate::integration::util::{
    deploy_contract, extend_contract, test_address, DeployKind, DeployOptions, AUTH, HELLO_WORLD,
};

#[tokio::test]
async fn simulate() {
    let sandbox = &TestEnv::new();
    let salt = Some(String::from("A"));
    let xdr_base64_build_only = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            kind: DeployKind::BuildOnly,
            salt: salt.clone(),
            ..Default::default()
        },
    )
    .await;
    let tx_env =
        TransactionEnvelope::from_xdr_base64(&xdr_base64_build_only, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env.clone()).unwrap();
    let assembled_str = sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .write_stdin(xdr_base64_build_only.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let assembled = simulate_and_assemble_transaction(&sandbox.client(), &tx, None, None, None)
        .await
        .unwrap();
    let txn_env: TransactionEnvelope = assembled.transaction().clone().into();
    assert_eq!(
        txn_env.to_xdr_base64(Limits::none()).unwrap(),
        assembled_str
    );
}

#[tokio::test]
async fn simulate_auth_modes() {
    let sandbox = &TestEnv::new();
    let xdr_base64_build_only = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            kind: DeployKind::BuildOnly,
            salt: Some(String::from("B")),
            ..Default::default()
        },
    )
    .await;

    // The unset default and the recording modes assemble the deployer
    // authorization the CreateContract op requires.
    for args in [
        &[][..],
        &["--auth-mode=root"][..],
        &["--auth-mode=non-root"][..],
    ] {
        sandbox
            .new_assert_cmd("tx")
            .arg("simulate")
            .args(args)
            .write_stdin(xdr_base64_build_only.as_bytes())
            .assert()
            .success();
    }

    // `enforce` only validates authorization already present on the envelope.
    // The build-only envelope has none, so it cannot authorize the deploy.
    sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .arg("--auth-mode=enforce")
        .write_stdin(xdr_base64_build_only.as_bytes())
        .assert()
        .failure()
        .stderr(predicates::str::contains("Auth, InvalidAction"));
}

// Regression test for https://github.com/stellar/stellar-cli/issues/2603:
// re-simulating an already-assembled envelope must not drop its recorded
// authorization or re-add the resource fee that is already folded into the
// transaction fee. This exercises the full CLI + RPC + XDR round trip that the
// `assemble` unit tests cannot reach.
#[tokio::test]
async fn simulate_reassembly_preserves_auth_and_fee() {
    let sandbox = &TestEnv::new();

    // Deploy the auth contract with the source account as the authorizer so the
    // recorded auth uses source-account credentials and re-simulation (which
    // defaults to enforce once entries exist) does not require a signature.
    let contract_id = sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--source=test")
        .arg("--wasm")
        .arg(AUTH.path())
        .arg("--")
        .arg("--addr=test")
        .assert()
        .success()
        .stdout_as_str();
    extend_contract(sandbox, &contract_id).await;

    let build_only = sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--build-only",
            "--source=test",
            "--id",
            &contract_id,
            "--",
            "do-auth",
            "--addr=test",
            "--val=hello",
        ])
        .assert()
        .success()
        .stdout_as_str();

    // First simulate: records auth and folds the resource fee into the tx fee.
    let assembled = sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .write_stdin(build_only.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Second simulate on the already-assembled envelope: must be idempotent.
    let reassembled = sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .write_stdin(assembled.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let auth_len = |xdr: &str| -> usize {
        let env = TransactionEnvelope::from_xdr_base64(xdr, Limits::none()).unwrap();
        let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(env).unwrap();
        let OperationBody::InvokeHostFunction(ref op) = tx.operations[0].body else {
            panic!("expected InvokeHostFunction operation");
        };
        op.auth.len()
    };
    let tx_of = |xdr: &str| {
        let env = TransactionEnvelope::from_xdr_base64(xdr, Limits::none()).unwrap();
        soroban_cli::commands::tx::xdr::unwrap_envelope_v1(env).unwrap()
    };

    // Auth recorded on the first pass must survive the second pass.
    assert!(auth_len(&assembled) > 0, "first simulate recorded no auth");
    assert_eq!(
        auth_len(&assembled),
        auth_len(&reassembled),
        "re-simulation dropped recorded auth"
    );

    // Re-simulating must not double-count the resource fee already folded into
    // the assembled transaction. The fee may still move by a few stroops (the
    // reassembled envelope is marginally larger), so the guard is that the fee
    // does not grow by anything close to another whole resource fee — the
    // pre-fix behavior that #2603 reported.
    let assembled_tx = tx_of(&assembled);
    let TransactionExt::V1(SorobanTransactionData { resource_fee, .. }) = &assembled_tx.ext else {
        panic!("assembled transaction is missing SorobanTransactionData");
    };
    let assembled_fee = assembled_tx.fee;
    let reassembled_fee = tx_of(&reassembled).fee;
    assert!(
        i64::from(reassembled_fee) < i64::from(assembled_fee) + resource_fee / 2,
        "re-simulation re-added the resource fee: assembled={assembled_fee}, \
         reassembled={reassembled_fee}, resource_fee={resource_fee}"
    );
}

fn test_tx_string(sandbox: &TestEnv) -> String {
    sandbox
        .new_assert_cmd("contract")
        .arg("upload")
        .args([
            "--wasm",
            HELLO_WORLD.path().as_os_str().to_str().unwrap(),
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str()
}

#[tokio::test]
async fn sequence_number_next() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
    let test = test_address(sandbox);
    let client = sandbox.network.rpc_client().unwrap();
    let test_account = client.get_account(&test).await.unwrap();
    let test_account_seq_num = test_account.seq_num.as_ref();

    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("update")
        .arg("seq-num")
        .arg("next")
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let updated_tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(updated_tx_env).unwrap();
    assert_eq!(
        tx.seq_num,
        soroban_cli::xdr::SequenceNumber(test_account_seq_num + 1)
    );
}

#[tokio::test]
async fn txn_hash() {
    let sandbox = &TestEnv::new();

    let xdr_base64 = "AAAAAgAAAACVk/0xt9tV/cUbF53iwQ3tkKLlq9zG2wV5qd9lRjZjlQAHt/sAFsKTAAAABAAAAAEAAAAAAAAAAAAAAABmOg6nAAAAAAAAAAEAAAAAAAAAGAAAAAAAAAABfcHs35M1GZ/JkY2+DHMs4dEUaqjynMnDYK/Gp0eulN8AAAAIdHJhbnNmZXIAAAADAAAAEgAAAAEFO1FR2Wg49QFY5KPOFAQ0bV5fN+7LD2GSQvOaHSH44QAAABIAAAAAAAAAAJWT/TG321X9xRsXneLBDe2QouWr3MbbBXmp32VGNmOVAAAACgAAAAAAAAAAAAAAADuaygAAAAABAAAAAQAAAAEFO1FR2Wg49QFY5KPOFAQ0bV5fN+7LD2GSQvOaHSH44QAAAY9SyLSVABbC/QAAABEAAAABAAAAAwAAAA8AAAASYXV0aGVudGljYXRvcl9kYXRhAAAAAAANAAAAJUmWDeWIDoxodDQXD2R2YFuP5K65ooYyx5lc87qDHZdjHQAAAAAAAAAAAAAPAAAAEGNsaWVudF9kYXRhX2pzb24AAAANAAAAcnsidHlwZSI6IndlYmF1dGhuLmdldCIsImNoYWxsZW5nZSI6ImhnMlRhOG8wWTliWFlyWlMyZjhzWk1kRFp6ektCSXhQNTZSd1FaNE90bTgiLCJvcmlnaW4iOiJodHRwOi8vbG9jYWxob3N0OjQ1MDcifQAAAAAADwAAAAlzaWduYXR1cmUAAAAAAAANAAAAQBcpuTFMxzkAdBs+5VIyJCBHaNuwEAva+kZVET4YuHVKF8gNII567RhxsnhBBSo5dDvssTN6vf2i42eEty66MtoAAAAAAAAAAX3B7N+TNRmfyZGNvgxzLOHRFGqo8pzJw2CvxqdHrpTfAAAACHRyYW5zZmVyAAAAAwAAABIAAAABBTtRUdloOPUBWOSjzhQENG1eXzfuyw9hkkLzmh0h+OEAAAASAAAAAAAAAACVk/0xt9tV/cUbF53iwQ3tkKLlq9zG2wV5qd9lRjZjlQAAAAoAAAAAAAAAAAAAAAA7msoAAAAAAAAAAAEAAAAAAAAAAwAAAAYAAAABfcHs35M1GZ/JkY2+DHMs4dEUaqjynMnDYK/Gp0eulN8AAAAUAAAAAQAAAAYAAAABBTtRUdloOPUBWOSjzhQENG1eXzfuyw9hkkLzmh0h+OEAAAAUAAAAAQAAAAeTiL4Gr2piUAmsXTev1ZzJ4kE2NUGZ0QMObd05iAMyzAAAAAMAAAAGAAAAAX3B7N+TNRmfyZGNvgxzLOHRFGqo8pzJw2CvxqdHrpTfAAAAEAAAAAEAAAACAAAADwAAAAdCYWxhbmNlAAAAABIAAAABBTtRUdloOPUBWOSjzhQENG1eXzfuyw9hkkLzmh0h+OEAAAABAAAAAAAAAACVk/0xt9tV/cUbF53iwQ3tkKLlq9zG2wV5qd9lRjZjlQAAAAYAAAABBTtRUdloOPUBWOSjzhQENG1eXzfuyw9hkkLzmh0h+OEAAAAVAAABj1LItJUAAAAAAEyTowAAGMgAAAG4AAAAAAADJBsAAAABRjZjlQAAAEASFnAIzNqpfdzv6yT0rSLMUDFgt7a/inCHurNCG55Jp8Imho04qRH+JNdkq0BgMC7yAJqH4N6Y2iGflFt3Lp4L";

    let expected_hash = "bcc9fa60c8f6607c981d6e1c65d77ae07617720113f9080fe5883d8e4a331a68";

    let hash = sandbox
        .new_assert_cmd("tx")
        .arg("hash")
        .write_stdin(xdr_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    assert_eq!(hash.trim(), expected_hash);
}

#[tokio::test]
async fn build_simulate_sign_send() {
    let sandbox = &TestEnv::new();
    // Generate a fresh account that hasn't been used yet to avoid sequence number conflicts
    sandbox.generate_account("fresh", None).assert().success();
    build_sim_sign_send(sandbox, "fresh", "--sign-with-key=fresh").await;
}

pub(crate) async fn build_sim_sign_send(sandbox: &TestEnv, account: &str, sign_with: &str) {
    // First deploy a contract normally so we have something to invoke
    let contract_id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some(account.to_string()),
            ..Default::default()
        },
    )
    .await;

    // Now build an invoke transaction that can be safely simulated and sent
    let xdr_base64_build_only = sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--build-only",
            "--id",
            &contract_id,
            "--source",
            account,
            "--",
            "hello",
            "--world",
            "test",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let tx_simulated = sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .write_stdin(xdr_base64_build_only.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    dbg!("{tx_simulated}");

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .arg("sign")
        .arg(sign_with)
        .write_stdin(tx_simulated.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    dbg!("{tx_signed}");

    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success()
        .stdout(predicates::str::contains("SUCCESS"));
}
