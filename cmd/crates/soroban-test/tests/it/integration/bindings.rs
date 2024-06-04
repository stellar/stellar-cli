use super::util::deploy_swap;
use super::util::deploy_custom_account;
use soroban_test::{TestEnv, LOCAL_NETWORK_PASSPHRASE};

const OUTPUT_DIR: &str = "./bindings-output";

#[tokio::test]
async fn invoke_test_generate_typescript_bindings() {
    let sandbox = &TestEnv::new();
    let contract_id = deploy_swap(sandbox).await;
    let outdir = sandbox.dir().join(OUTPUT_DIR);
    let cmd = sandbox.cmd_arr::<soroban_cli::commands::contract::bindings::typescript::Cmd>(&[
        "--network-passphrase",
        LOCAL_NETWORK_PASSPHRASE,
        "--rpc-url",
        &sandbox.rpc_url,
        "--output-dir",
        &outdir.display().to_string(),
        "--overwrite",
        "--contract-id",
        &contract_id.to_string(),
    ]);

    let result = sandbox.run_cmd_with(cmd, "test").await;

    assert!(result.is_ok(), "Failed to generate TypeScript bindings");

    assert!(outdir.exists(), "Output directory does not exist");

    let files = std::fs::read_dir(outdir).expect("Failed to read output directory");
    assert!(
        files.count() > 0,
        "No files generated in the output directory"
    );
}

#[tokio::test]
async fn invoke_test_bindings_context_failure() {
    let sandbox = &TestEnv::new();
    let contract_id = deploy_custom_account(sandbox).await;
    let outdir = sandbox.dir().join(OUTPUT_DIR);
    let cmd = sandbox.cmd_arr::<soroban_cli::commands::contract::bindings::typescript::Cmd>(&[
        "--network-passphrase",
        LOCAL_NETWORK_PASSPHRASE,
        "--rpc-url",
        &sandbox.rpc_url,
        "--output-dir",
        &outdir.display().to_string(),
        "--overwrite",
        "--contract-id",
        &contract_id.to_string(),
    ]);

    let result = sandbox.run_cmd_with(cmd, "test").await;

    assert!(result.is_ok(), "Failed to generate TypeScript bindings");

    assert!(outdir.exists(), "Output directory does not exist");

    let files = std::fs::read_dir(outdir).expect("Failed to read output directory");
    assert!(
        files.count() > 0,
        "No files generated in the output directory"
    );
}