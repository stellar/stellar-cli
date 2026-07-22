use predicates::prelude::{predicate, PredicateBooleanExt};
use soroban_test::AssertExt;
use soroban_test::TestEnv;

fn pubkey_for_identity(sandbox: &TestEnv, name: &str) -> String {
    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg(name)
        .assert()
        .stdout_as_str()
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn fund() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("fund")
        .arg("test2")
        .assert()
        // Don't expect error if friendbot indicated that the account is
        // already fully funded to the starting balance, because the
        // user's goal is to get funded, and the account is funded
        // so it is success much the same.
        .success();
}

#[tokio::test]
async fn secret() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("secret")
        .arg("test2")
        .assert()
        .success();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn overwrite_identity() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .assert()
        .success();

    let initial_pubkey = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test2")
        .assert()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .assert()
        .stderr(predicate::str::contains(
            "error: An identity with the name 'test2' already exists",
        ));

    assert_eq!(initial_pubkey, pubkey_for_identity(sandbox, "test2"));

    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .arg("--overwrite")
        .assert()
        .stderr(predicate::str::contains("Overwriting identity 'test2'"))
        .success();

    assert_ne!(initial_pubkey, pubkey_for_identity(sandbox, "test2"));
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn overwrite_identity_with_add() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test3")
        .assert()
        .success();

    let initial_pubkey = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test3")
        .assert()
        .stdout_as_str();

    // Try to add a key with the same name, should fail
    sandbox
        .new_assert_cmd("keys")
        .arg("add")
        .arg("test3")
        .arg("--public-key")
        .arg("GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC")
        .assert()
        .stderr(predicate::str::contains(
            "error: An identity with the name 'test3' already exists",
        ));

    // Verify the key wasn't changed
    assert_eq!(initial_pubkey, pubkey_for_identity(sandbox, "test3"));

    // Try again with --overwrite flag, should succeed
    sandbox
        .new_assert_cmd("keys")
        .arg("add")
        .arg("test3")
        .arg("--public-key")
        .arg("GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC")
        .arg("--overwrite")
        .assert()
        .stderr(predicate::str::contains("Overwriting identity 'test3'"))
        .success();

    // Verify the key was changed
    assert_ne!(initial_pubkey, pubkey_for_identity(sandbox, "test3"));
    assert_eq!(
        "GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC",
        pubkey_for_identity(sandbox, "test3").trim()
    );
}

#[tokio::test]
async fn add_public_key_rejects_secret_bearing_input() {
    let sandbox = &TestEnv::new();
    let secret = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH";

    sandbox
        .new_assert_cmd("keys")
        .arg("add")
        .arg("public-only")
        .arg("--public-key")
        .arg(secret)
        .assert()
        .failure();

    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("public-only")
        .assert()
        .failure();

    let seed = "depth decade power loud smile spatial sign movie judge february rate broccoli";
    sandbox
        .new_assert_cmd("keys")
        .arg("add")
        .arg("public-only-seed")
        .arg("--public-key")
        .arg(seed)
        .assert()
        .failure();

    sandbox
        .new_assert_cmd("keys")
        .arg("add")
        .arg("public-only-ledger")
        .arg("--public-key")
        .arg("ledger")
        .assert()
        .failure();

    sandbox
        .new_assert_cmd("keys")
        .arg("add")
        .arg("public-only-secure")
        .arg("--public-key")
        .arg("secure_store:org.stellar.cli-alice")
        .assert()
        .failure();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn set_default_identity() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test4")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("use")
        .arg("test4")
        .assert()
        .stderr(predicate::str::contains(
            "The default source account is set to `test4`",
        ))
        .success();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn unset_default_identity() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test5")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("use")
        .arg("test5")
        .assert()
        .stderr(predicate::str::contains(
            "The default source account is set to `test5`",
        ))
        .success();

    sandbox
        .new_assert_cmd("env")
        .env_remove("STELLAR_ACCOUNT")
        .assert()
        .stdout(predicate::str::contains("STELLAR_ACCOUNT=test5"))
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("unset")
        .assert()
        .stderr(predicate::str::contains(
            "The default source account has been unset",
        ))
        .success();

    sandbox
        .new_assert_cmd("env")
        .env_remove("STELLAR_ACCOUNT")
        .assert()
        .stdout(predicate::str::contains("STELLAR_ACCOUNT=").not())
        .success();
}

#[tokio::test]
async fn rm_requires_confirmation() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("rmtest1")
        .assert()
        .success();

    // Piping "n" should cancel removal
    sandbox
        .new_assert_cmd("keys")
        .arg("rm")
        .arg("rmtest1")
        .write_stdin("n\n")
        .assert()
        .stderr(predicate::str::contains("removal cancelled by user"))
        .failure();

    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("rmtest1")
        .assert()
        .success();

    // Piping empty input (just Enter) should default to cancel
    sandbox
        .new_assert_cmd("keys")
        .arg("rm")
        .arg("rmtest1")
        .write_stdin("\n")
        .assert()
        .stderr(predicate::str::contains("removal cancelled by user"))
        .failure();

    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("rmtest1")
        .assert()
        .success();

    // Piping "y" should confirm removal
    sandbox
        .new_assert_cmd("keys")
        .arg("rm")
        .arg("rmtest1")
        .write_stdin("y\n")
        .assert()
        .stderr(predicate::str::contains(
            "Removing the key's cli config file",
        ))
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("rmtest1")
        .assert()
        .failure();
}

#[tokio::test]
async fn rm_with_force_skips_confirmation() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("rmtest2")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("rm")
        .arg("rmtest2")
        .arg("--force")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("rmtest2")
        .assert()
        .failure();
}

// `keys generate --hd-path N` (plain seed-phrase storage) must persist N so
// that later `keys address` calls without `--hd-path` derive at index N rather
// than the default. Guards the user-visible contract from #2538 across CLI
// parsing, identity-file I/O, and key derivation.
#[tokio::test]
async fn hd_path_persists_for_keys_generate() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "hd-gen", "--hd-path", "5"])
        .assert()
        .success();

    let address_default = pubkey_for_identity(sandbox, "hd-gen");
    let address_explicit = sandbox
        .new_assert_cmd("keys")
        .args(["address", "hd-gen", "--hd-path", "5"])
        .assert()
        .success()
        .stdout_as_str();
    let address_zero = sandbox
        .new_assert_cmd("keys")
        .args(["address", "hd-gen", "--hd-path", "0"])
        .assert()
        .success()
        .stdout_as_str();

    assert_eq!(
        address_default, address_explicit,
        "expected `keys address hd-gen` (no flag) to derive at the persisted hd_path 5"
    );
    assert_ne!(
        address_default, address_zero,
        "expected hd_path 5 derivation to differ from hd_path 0"
    );
}

#[tokio::test]
async fn hd_path_persists_for_keys_add_seed_phrase() {
    let sandbox = &TestEnv::new();
    let seed_phrase = "aisle reflect depart add safe fury dress artist bronze abuse warrior clap inquiry ask mandate deputy view trade debate flip priority boy depart recipe";

    sandbox
        .new_assert_cmd("keys")
        .write_stdin(format!("{seed_phrase}\n"))
        .args(["add", "hd-add", "--hd-path", "5"])
        .assert()
        .success();

    let address_default = pubkey_for_identity(sandbox, "hd-add");
    let address_explicit = sandbox
        .new_assert_cmd("keys")
        .args(["address", "hd-add", "--hd-path", "5"])
        .assert()
        .success()
        .stdout_as_str();
    let address_zero = sandbox
        .new_assert_cmd("keys")
        .args(["address", "hd-add", "--hd-path", "0"])
        .assert()
        .success()
        .stdout_as_str();

    assert_eq!(
        address_default, address_explicit,
        "expected `keys address hd-add` (no flag) to derive at the persisted hd_path 5"
    );
    assert_ne!(
        address_default, address_zero,
        "expected hd_path 5 derivation to differ from hd_path 0"
    );
}

#[tokio::test]
async fn rm_nonexistent_key() {
    let sandbox = &TestEnv::new();

    // Without --force: should fail before prompting
    sandbox
        .new_assert_cmd("keys")
        .arg("rm")
        .arg("doesnotexist")
        .assert()
        .failure();

    // With --force: should still fail
    sandbox
        .new_assert_cmd("keys")
        .arg("rm")
        .arg("doesnotexist")
        .arg("--force")
        .assert()
        .failure();
}
