//! Coverage for `contract build --image --pull`. Requires a working engine, so
//! it lives in the opt-in `integration` suite (run with `--features it`).

use predicates::prelude::predicate;
use soroban_test::TestEnv;
use std::path::PathBuf;

// A failed pull surfaces the engine error as `PullImageFailed`. Uses a
// well-formed but nonexistent image so the pull fails fast (no large download),
// before any build/mount happens.
#[test]
fn build_image_pull_reports_failure() {
    let sandbox = TestEnv::default();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workspace");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture)
        .arg("build")
        .arg("--image")
        .arg("docker.io/library/stellar-cli-does-not-exist-zzz:latest")
        .arg("--pull")
        .assert()
        .failure()
        .stderr(predicate::str::contains("could not pull image"));
}
