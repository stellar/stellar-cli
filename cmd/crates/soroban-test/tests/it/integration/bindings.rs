use super::util::deploy_custom_account;
use super::util::deploy_swap;
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
        &sandbox.network.rpc_url,
        "--output-dir",
        &outdir.display().to_string(),
        "--overwrite",
        "--contract-id",
        &contract_id.to_string(),
    ]);

    let result = cmd.execute(false).await;

    assert!(result.is_ok(), "Failed to generate TypeScript bindings");

    assert!(outdir.exists(), "Output directory does not exist");

    let files = std::fs::read_dir(outdir).expect("Failed to read output directory");
    assert!(
        files.count() > 0,
        "No files generated in the output directory"
    );
}

#[tokio::test]
async fn typescript_bindings_do_not_render_operator_rpc_url() {
    let sandbox = &TestEnv::new();
    let contract_id = deploy_swap(sandbox).await;
    let outdir = sandbox.dir().join(OUTPUT_DIR);
    // A credentialed RPC URL pointing at the local node; the userinfo is ignored
    // by the node but must never be written into the generated package.
    let credentialed_rpc =
        sandbox
            .network
            .rpc_url
            .replacen("http://", "http://alice:supersecret@", 1);
    let cmd = sandbox.cmd_arr::<soroban_cli::commands::contract::bindings::typescript::Cmd>(&[
        "--network-passphrase",
        LOCAL_NETWORK_PASSPHRASE,
        "--rpc-url",
        &credentialed_rpc,
        "--output-dir",
        &outdir.display().to_string(),
        "--overwrite",
        "--contract-id",
        &contract_id.to_string(),
    ]);

    cmd.execute(false)
        .await
        .expect("Failed to generate TypeScript bindings");

    // The operator's RPC URL (and any credentials in it) is never rendered; a
    // local default is used instead.
    let readme = std::fs::read_to_string(outdir.join("README.md")).expect("README.md missing");
    assert!(!readme.contains("supersecret"));
    assert!(!readme.contains(&credentialed_rpc));
    assert!(readme.contains("http://localhost:8000/rpc"));

    // The public contract address is still embedded via the networks export that
    // consumers and the README example rely on.
    let index_ts =
        std::fs::read_to_string(outdir.join("src/index.ts")).expect("src/index.ts missing");
    assert!(index_ts.contains("export const networks"));
    assert!(index_ts.contains(&contract_id));
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
        &sandbox.network.rpc_url,
        "--output-dir",
        &outdir.display().to_string(),
        "--overwrite",
        "--contract-id",
        &contract_id.to_string(),
    ]);

    let result = cmd.execute(false).await;

    assert!(result.is_ok(), "Failed to generate TypeScript bindings");

    assert!(outdir.exists(), "Output directory does not exist");

    let files = std::fs::read_dir(&outdir).expect("Failed to read output directory");
    assert!(
        files.count() > 0,
        "No files generated in the output directory"
    );
    // Read the src/index.ts file and check for `__check_auth:`
    let index_ts_path = outdir.join("src/index.ts");

    assert!(index_ts_path.exists(), "src/index.ts file does not exist");

    let content = std::fs::read_to_string(&index_ts_path).expect("Failed to read index.ts file");
    // `__check_auth` is a host-only function and must not be exposed as a client
    // method. It may still be referenced in doc comments (e.g. on the SDK
    // `Context` type), so assert on the generated method signature specifically.
    assert!(
        !content.contains("__check_auth: ("),
        "Test failed: `__check_auth` exposed as a client method in src/index.ts"
    );

    // check enum message + doc working properly
    assert!(
        content.contains("The requested item was not found.")
            && content.contains("1: {message:\"NotFound\"}"),
        r#"Test failed: Error enum not properly formatted in src/index.ts"#
    );
}
