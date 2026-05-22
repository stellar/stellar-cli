//! End-to-end tests for `stellar contract verify`.
//!
//! Pipeline, entirely through the cli (no git/network clone needed):
//!   1. `contract init` scaffolds a workspace + `hello-world` contract.
//!   2. `contract build --verifiable` builds it against a pinned bldimg and
//!      records `source_sha256` in the wasm's SEP-58 metadata.
//!   3. `contract archive` regenerates the *same* source tarball (same
//!      `build_source_archive` the verifiable build used), so its sha256 matches
//!      the recorded `source_sha256`.
//!   4. `contract verify --source-uri <that archive>` materializes the source,
//!      rebuilds in the bldimg, and byte-compares.
//!
//! The happy-path tests require docker + the pinned bldimg pullable from Docker
//! Hub. They are always-run by convention (per the project's "no #[ignore]"
//! rule) — failures there flag a regression or pinned-resource drift loudly.
//!
//! Fixture pin:
//!   - bldimg: `docker.io/fnando/stellar-cli-experimental@sha256:85e76e…`.
//!     TODO: swap to `docker.io/stellar/stellar-cli@sha256:<…>` once
//!     `stellar/stellar-cli-docker` publishes a canonical tag matching the
//!     cli version under test.

use predicates::prelude::{predicate, PredicateBooleanExt};
use soroban_test::TestEnv;
use std::path::{Path, PathBuf};

const PINNED_BLDIMG: &str =
    "docker.io/fnando/stellar-cli-experimental@sha256:85e76eae8bf9f47ba94391214b76f8fa2b9d7b28171774dfafaf5b8d613a74d3";

/// Scaffold a workspace with the default `hello-world` contract under
/// `<sandbox>/proj`. The scaffolded tree is not a git repo, so the verifiable
/// build archives the working directory directly.
fn init_project(sandbox: &TestEnv) -> PathBuf {
    let proj = sandbox.dir().join("proj");
    sandbox
        .new_assert_cmd("contract")
        .arg("init")
        .arg(&proj)
        .assert()
        .success();
    proj
}

/// Build the scaffolded contract verifiably and generate the matching source
/// archive. Returns `(wasm_path, archive_path)`.
///
/// The archive is produced *after* the verifiable build on purpose: the build's
/// host-side `cargo metadata` writes `Cargo.lock` into the workspace, and
/// `contract archive` then captures that same tree — so the archive's sha256
/// equals the `source_sha256` the build recorded into the wasm.
fn build_and_archive(sandbox: &TestEnv, proj: &Path) -> (PathBuf, PathBuf) {
    let out_dir = sandbox.dir().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    sandbox
        .new_assert_cmd("contract")
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg(PINNED_BLDIMG)
        .arg("--out-dir")
        .arg(&out_dir)
        .current_dir(proj)
        .assert()
        .success();

    let archive = sandbox.dir().join("source.tar.gz");
    sandbox
        .new_assert_cmd("contract")
        .arg("archive")
        .arg("--out-file")
        .arg(&archive)
        .current_dir(proj)
        .assert()
        .success();

    (out_dir.join("hello_world.wasm"), archive)
}

/// Happy path: build a verifiable wasm, then verify it from the local file,
/// handing the cli the matching source archive via `--source-uri`. Asserts the
/// cli prints `Verified:` on stderr.
#[test]
fn verify_wasm_succeeds_for_freshly_built_verifiable_wasm() {
    let sandbox = TestEnv::default();
    let proj = init_project(&sandbox);
    let (wasm, archive) = build_and_archive(&sandbox, &proj);

    sandbox
        .new_assert_cmd("contract")
        .arg("verify")
        .arg("--wasm")
        .arg(&wasm)
        .arg("--source-uri")
        .arg(&archive)
        .arg("--trust")
        .assert()
        .success()
        .stderr(predicate::str::contains("Verified:"));
}

/// Build verifiable → upload to local network → verify by `--id`. Exercises the
/// `wasm::fetch_from_contract` path through the verify command.
#[tokio::test]
async fn verify_id_succeeds_after_upload() {
    let sandbox = TestEnv::new();
    let proj = init_project(&sandbox);
    let (wasm, archive) = build_and_archive(&sandbox, &proj);
    let wasm_str = wasm.to_string_lossy().to_string();

    // Deploy gives us a contract id `--id` can resolve to the on-ledger wasm.
    let id = sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--wasm")
        .arg(&wasm_str)
        .arg("--alias")
        .arg("verify_e2e")
        .arg("--ignore-checks")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not())
        .get_output()
        .stdout
        .clone();
    let id = String::from_utf8(id).unwrap().trim().to_string();

    sandbox
        .new_assert_cmd("contract")
        .arg("verify")
        .arg("--id")
        .arg(&id)
        .arg("--source-uri")
        .arg(&archive)
        .arg("--trust")
        .assert()
        .success()
        .stderr(predicate::str::contains("Verified:"));
}

/// Flip a byte in a verifiable wasm and confirm `contract verify` reports the
/// mismatch. The flipped byte is in the middle (code) so the trailing
/// `contractmetav0` section still parses; the rebuild reproduces the original
/// bytes, and the byte comparison fails.
#[test]
fn verify_wasm_fails_on_tampered_bytes() {
    let sandbox = TestEnv::default();
    let proj = init_project(&sandbox);
    let (wasm, archive) = build_and_archive(&sandbox, &proj);

    let mut bytes = std::fs::read(&wasm).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] = bytes[mid].wrapping_add(1);
    let tampered = sandbox.dir().join("tampered.wasm");
    std::fs::write(&tampered, &bytes).unwrap();

    sandbox
        .new_assert_cmd("contract")
        .arg("verify")
        .arg("--wasm")
        .arg(&tampered)
        .arg("--source-uri")
        .arg(&archive)
        .arg("--trust")
        .assert()
        .failure()
        .stderr(predicate::str::contains("verification failed"));
}
