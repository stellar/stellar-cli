use crate::integration::util::{deploy_contract, DeployOptions, HELLO_WORLD};

use soroban_test::{AssertExt, TestEnv};

// Exercises CAP-85 externally managed executables end-to-end:
//
// 1. Deploy a "beacon" contract (hello_world exposes the CAP-85 helpers).
// 2. Publish an executable reference entry `fleet` -> hello_world wasm hash.
// 3. Deploy a proxy contract whose executable is an `ExternalRef` to `fleet`.
// 4. Assert the CLI transparently resolves the reference for invoke / fetch /
//    info against the proxy, whose own contract instance has no direct wasm.
#[tokio::test]
async fn external_ref_resolves_for_invoke_fetch_and_info() {
    let sandbox = &TestEnv::new();
    let wasm_bytes = HELLO_WORLD.bytes();
    let wasm_hash = HELLO_WORLD.hash().unwrap().to_string();

    // Deploy the beacon. Deploying also uploads the hello_world wasm, so the
    // hash the reference entry points at already exists on-ledger.
    let beacon_id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some("test".to_string()),
            ..Default::default()
        },
    )
    .await;

    // Publish the executable reference entry keyed by `fleet`.
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &beacon_id,
            "--",
            "publish",
            "--tag",
            "fleet",
            "--wasm_hash",
            &wasm_hash,
        ])
        .assert()
        .success();

    // get_ref round-trips the published hash.
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--is-view",
            "--id",
            &beacon_id,
            "--",
            "get_ref",
            "--tag",
            "fleet",
        ])
        .assert()
        .success()
        .stdout(format!("\"{wasm_hash}\"\n"));

    // Deploy a proxy whose executable is an `ExternalRef` to `fleet`.
    let proxy_out = sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &beacon_id,
            "--",
            "deploy_ref",
            "--tag",
            "fleet",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let proxy_id = proxy_out.trim().trim_matches('"');

    // invoke against the proxy: the CLI must resolve the ExternalRef to fetch
    // the underlying wasm's spec and run its code.
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--is-view",
            "--id",
            proxy_id,
            "--",
            "hello",
            "--world=world",
        ])
        .assert()
        .success()
        .stdout("[\"Hello\",\"world\"]\n");

    // fetch against the proxy returns the referenced wasm bytes.
    sandbox
        .new_assert_cmd("contract")
        .args(["fetch", "--id", proxy_id])
        .assert()
        .success()
        .stdout(predicates::ord::eq(wasm_bytes));

    // info interface against the proxy succeeds (spec resolved via ExternalRef).
    sandbox
        .new_assert_cmd("contract")
        .args(["info", "interface", "--id", proxy_id])
        .assert()
        .success()
        .stdout(predicates::str::contains("fn hello"));

    // info hash against the proxy resolves the ExternalRef to the referenced
    // wasm hash (a distinct code path: fetch_wasm_hash_from_contract).
    sandbox
        .new_assert_cmd("contract")
        .args(["info", "hash", "--id", proxy_id])
        .assert()
        .success()
        .stdout(format!("{wasm_hash}\n"));
}
