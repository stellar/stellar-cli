mod e2e_rpc_server;
mod integration_test;
mod invoke_sandbox;

#[ctor::ctor]
fn build_tests() {
    let mut cmd = soroban_cli::build::Cmd::optimized();
    cmd.cargo.workspace.workspace = true;
    cmd.cargo.workspace.exclude.push("soroban-cli".to_string());
    cmd.run().expect("failed to compile tests");
    
    print!("Successfully compiled")
}