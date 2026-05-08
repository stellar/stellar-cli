use predicates::prelude::predicate;
use soroban_cli::tx::ONE_XLM;
use soroban_test::{AssertExt, TestEnv};

// All secure store tests are run within one test to avoid issues with multiple
// tests trying to access the dbus at the same time which can lead to intermittent failures.
#[tokio::test]
async fn secure_store_key_management() {
    let sandbox = &TestEnv::new();

    let secure_key_name = "secure-store-test";

    // generate a new secret key in secure store
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", secure_key_name, "--secure-store", "--fund"])
        .assert()
        .success();

    // validate that we cannot get the secret key back
    sandbox
        .new_assert_cmd("keys")
        .arg("secret")
        .arg(secure_key_name)
        .assert()
        .stderr(predicate::str::contains("does not reveal secret key"))
        .failure();

    // validate that we can get the public key
    let secure_store_address = sandbox
        .new_assert_cmd("keys")
        .args(["address", secure_key_name])
        .assert()
        .success()
        .stdout_as_str();
    assert!(secure_store_address.starts_with('G'));

    // validate that the public key is cached on disk (so `keys address` and
    // tx-signing hint derivation can skip the keychain on subsequent calls).
    let identity_path = sandbox
        .config_dir()
        .join("identity")
        .join(format!("{secure_key_name}.toml"));
    let identity_toml = std::fs::read_to_string(&identity_path).unwrap_or_else(|err| {
        panic!("expected identity file at {identity_path:?}: {err}");
    });
    assert!(
        identity_toml.contains(&format!("public_key = \"{secure_store_address}\"")),
        "expected public_key to be cached on disk after `keys generate --secure-store`, \
         but identity file was:\n{identity_toml}"
    );

    // use the secure store key to fund a new account
    let new_key_name = "new";
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", new_key_name])
        .assert()
        .success();
    let new_address = sandbox
        .new_assert_cmd("keys")
        .args(["address", new_key_name])
        .assert()
        .success()
        .stdout_as_str();

    let client = sandbox.network.rpc_client().unwrap();
    let secure_account = client.get_account(&secure_store_address).await.unwrap();

    let starting_balance = ONE_XLM * 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-account",
            "--destination",
            new_address.as_str(),
            "--starting-balance",
            starting_balance.to_string().as_str(),
            "--source",
            secure_key_name,
        ])
        .assert()
        .success()
        .stdout_as_str();

    let secure_account_after = client.get_account(&secure_store_address).await.unwrap();
    assert!(secure_account_after.balance < secure_account.balance);

    let new_account = client.get_account(&new_address).await.unwrap();
    assert_eq!(new_account.balance, starting_balance);

    // generating the same key again without --overwrite must fail
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", secure_key_name, "--secure-store"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    // generating the same key again with --overwrite must succeed and replace the entry
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", secure_key_name, "--secure-store", "--overwrite"])
        .assert()
        .success();

    // the address should still be a valid public key (new key was written)
    let new_secure_store_address = sandbox
        .new_assert_cmd("keys")
        .args(["address", secure_key_name])
        .assert()
        .success()
        .stdout_as_str();
    assert!(new_secure_store_address.starts_with('G'));

    // `keys add --secure-store --hd-path N` must derive and cache the public
    // key at the requested HD path, and persist `hd_path` in the identity TOML.
    let seed_phrase = "aisle reflect depart add safe fury dress artist bronze abuse warrior clap inquiry ask mandate deputy view trade debate flip priority boy depart recipe";

    let add_default = "secure-store-add-default";
    sandbox
        .new_assert_cmd("keys")
        .write_stdin(format!("{seed_phrase}\n"))
        .args(["add", add_default, "--secure-store"])
        .assert()
        .success();
    let address_default = sandbox
        .new_assert_cmd("keys")
        .args(["address", add_default])
        .assert()
        .success()
        .stdout_as_str();

    let add_hd_path = "secure-store-add-hd-path";
    sandbox
        .new_assert_cmd("keys")
        .write_stdin(format!("{seed_phrase}\n"))
        .args(["add", add_hd_path, "--secure-store", "--hd-path", "5"])
        .assert()
        .success();
    let address_hd_path = sandbox
        .new_assert_cmd("keys")
        .args(["address", add_hd_path, "--hd-path", "5"])
        .assert()
        .success()
        .stdout_as_str();

    assert!(address_hd_path.starts_with('G'));
    assert_ne!(
        address_default, address_hd_path,
        "expected --hd-path 5 to derive a different public key than the default path"
    );

    let identity_path = sandbox
        .config_dir()
        .join("identity")
        .join(format!("{add_hd_path}.toml"));
    let identity_toml = std::fs::read_to_string(&identity_path).unwrap_or_else(|err| {
        panic!("expected identity file at {identity_path:?}: {err}");
    });
    assert!(
        identity_toml.contains("hd_path = 5"),
        "expected hd_path = 5 to be persisted after `keys add --secure-store --hd-path 5`, \
         but identity file was:\n{identity_toml}"
    );
    assert!(
        identity_toml.contains(&format!("public_key = \"{}\"", address_hd_path.trim())),
        "expected cached public_key to match the address derived at hd_path 5, \
         but identity file was:\n{identity_toml}"
    );

    // Strip the cached public_key but keep hd_path = 5: simulates a legacy
    // identity that persisted hd_path but never cached the derived pubkey.
    // `keys address` without --hd-path must rederive at the persisted index 5
    // and write the index-5 pubkey back to the cache — not silently overwrite
    // it with the index-0 pubkey.
    let stripped: String = identity_toml
        .lines()
        .filter(|line| !line.trim_start().starts_with("public_key"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&identity_path, stripped).unwrap();

    let address_after_rederive = sandbox
        .new_assert_cmd("keys")
        .args(["address", add_hd_path])
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(
        address_after_rederive.trim(),
        address_hd_path.trim(),
        "expected `keys address {add_hd_path}` (no --hd-path) to derive at the persisted hd_path 5"
    );

    let identity_toml = std::fs::read_to_string(&identity_path).unwrap();
    assert!(
        identity_toml.contains(&format!("public_key = \"{}\"", address_hd_path.trim())),
        "expected cache write-back to use the persisted hd_path, \
         but identity file was:\n{identity_toml}"
    );
}
