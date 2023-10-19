use soroban_cli::commands::{
    config::identity,
    contract::{self, fetch},
};
use soroban_test::TestEnv;
use std::path::PathBuf;

use crate::util::{
    add_test_seed, is_rpc, network_passphrase, network_passphrase_arg, rpc_url, rpc_url_arg,
    DEFAULT_PUB_KEY, DEFAULT_PUB_KEY_1, DEFAULT_SECRET_KEY, DEFAULT_SEED_PHRASE, HELLO_WORLD,
    TEST_SALT,
};








#[tokio::test]
async fn fetch() {
    if !is_rpc() {
        return;
    }
    let e = TestEnv::default();
    let f = e.dir().join("contract.wasm");
    let id = deploy_hello(&e);
    let cmd = e.cmd_arr::<fetch::Cmd>(&["--id", &id, "--out-file", f.to_str().unwrap()]);
    cmd.run().await.unwrap();
    assert!(f.exists());
}
