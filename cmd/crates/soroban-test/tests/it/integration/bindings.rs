use soroban_test::{TestEnv, LOCAL_NETWORK_PASSPHRASE};
use super::util::deploy_swap;

pub const OUTPUT_DIR: &str = "./bindings-output";


#[tokio::test]
async fn invoke_test_generate_typescript_bindings() {
    let sandbox = &TestEnv::new();
    let contract_id = deploy_swap(sandbox).await;
    let cmd = sandbox.cmd_arr::<soroban_cli::commands::contract::bindings::typescript::Cmd>(&[
        "--network-passphrase",
        LOCAL_NETWORK_PASSPHRASE,
        "--rpc-url",
        &sandbox.rpc_url,
        "--output-dir",
        OUTPUT_DIR,
        "--overwrite",
        "--contract-id",
        &contract_id.to_string(),
    ]);

    let result = sandbox.run_cmd_with(cmd, "test").await;

    assert!(result.is_ok(), "Failed to generate TypeScript bindings");

    let output_dir = std::path::Path::new(OUTPUT_DIR);
    assert!(output_dir.exists(), "Output directory does not exist");

    let files = std::fs::read_dir(output_dir).expect("Failed to read output directory");
    assert!(files.count() > 0, "No files generated in the output directory");
}
