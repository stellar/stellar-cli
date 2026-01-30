use soroban_test::{AssertExt, TestEnv};
use std::path::PathBuf;

#[tokio::test]
async fn deploy_without_wasm_auto_builds() {
    let sandbox = TestEnv::new();
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/test-wasms/hello_world");

    // Deploy without --wasm flag should auto-build the contract
    let contract_id = sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("deploy")
        .arg("--source-account")
        .arg("test")
        .assert()
        .success()
        .stdout_as_str();

    // Verify contract was deployed by invoking a function
    sandbox
        .invoke_with_test(&["--id", &contract_id, "--", "hello", "--world=world"])
        .await
        .unwrap();
}

#[tokio::test]
async fn deploy_workspace_without_package_builds_all() {
    let sandbox = TestEnv::new();
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");

    // Deploy in workspace without --wasm and without --package builds and deploys all contracts
    sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("deploy")
        .arg("--source-account")
        .arg("test")
        .assert()
        .success()
        .stderr(predicates::str::contains("Build Complete"));
}

#[tokio::test]
async fn deploy_workspace_with_package_auto_builds() {
    let sandbox = TestEnv::new();
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");

    // Deploy in workspace with --package should auto-build the specified contract
    let contract_id = sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("deploy")
        .arg("--source-account")
        .arg("test")
        .arg("--package")
        .arg("add")
        .assert()
        .success()
        .stdout_as_str();

    // Verify contract was deployed
    assert!(!contract_id.is_empty(), "Expected contract ID");
}

#[tokio::test]
async fn upload_without_wasm_auto_builds() {
    let sandbox = TestEnv::new();
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/test-wasms/hello_world");

    // Upload without --wasm flag should auto-build the contract
    let wasm_hash = sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("upload")
        .arg("--source-account")
        .arg("test")
        .assert()
        .success()
        .stdout_as_str();

    // Verify a hash was returned
    assert_eq!(wasm_hash.len(), 64, "Expected 64-character hex hash");
}

#[tokio::test]
async fn upload_workspace_without_package_builds_all() {
    let sandbox = TestEnv::new();
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");

    // Upload in workspace without --wasm and without --package builds and uploads all contracts
    sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("upload")
        .arg("--source-account")
        .arg("test")
        .assert()
        .success()
        .stderr(predicates::str::contains("Build Complete"));
}

#[tokio::test]
async fn upload_workspace_with_package_auto_builds() {
    let sandbox = TestEnv::new();
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");

    // Upload in workspace with --package should auto-build the specified contract
    let wasm_hash = sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("upload")
        .arg("--source-account")
        .arg("test")
        .arg("--package")
        .arg("call")
        .assert()
        .success()
        .stdout_as_str();

    // Verify a hash was returned
    assert_eq!(wasm_hash.len(), 64, "Expected 64-character hex hash");
}

#[tokio::test]
async fn deploy_outside_cargo_project_requires_wasm() {
    let sandbox = TestEnv::new();

    // Deploy outside a Cargo project without --wasm should fail
    sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--source-account")
        .arg("test")
        .assert()
        .failure()
        .stderr(predicates::str::contains("could not find `Cargo.toml`"));
}

#[tokio::test]
async fn upload_outside_cargo_project_requires_wasm() {
    let sandbox = TestEnv::new();

    // Upload outside a Cargo project without --wasm should fail
    sandbox
        .new_assert_cmd("contract")
        .arg("upload")
        .arg("--source-account")
        .arg("test")
        .assert()
        .failure()
        .stderr(predicates::str::contains("could not find `Cargo.toml`"));
}
