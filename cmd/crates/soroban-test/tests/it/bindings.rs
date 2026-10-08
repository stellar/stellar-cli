use predicates::prelude::predicate;
use soroban_test::TestEnv;

use crate::util::CUSTOM_TYPES;

// The deprecation warning has to say where the command is going and which SDK
// release provides the replacement, so users know what to install before the
// native command is removed.
#[test]
fn typescript_bindings_deprecation_warning_names_the_plugin() {
    let sandbox = TestEnv::default();

    sandbox
        .new_assert_cmd("contract")
        .args(["bindings", "typescript", "--wasm"])
        .arg(CUSTOM_TYPES.path())
        .args(["--output-dir", "bindings-output"])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "`stellar contract bindings typescript` is deprecated and will be removed",
        ))
        .stderr(predicate::str::contains(
            "`@stellar/stellar-sdk` 17.2.0 or later",
        ))
        .stderr(predicate::str::contains(
            "https://github.com/stellar/js-stellar-sdk#stellar-cli-plugin",
        ));
}
