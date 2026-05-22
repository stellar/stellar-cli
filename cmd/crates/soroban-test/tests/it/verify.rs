//! End-to-end tests for `stellar contract verify`.
//!
//! These exercise the full pipeline: build a contract verifiably against a
//! pinned bldimg + pinned source_repo, then verify the resulting wasm matches.
//! The "happy path" tests require docker + network access to GitHub + the
//! pinned bldimg pullable from Docker Hub. They are always-run by convention
//! (per the project's "no #[ignore]" rule) — failures there flag a regression
//! or pinned-resource drift loudly.
//!
//! Fixture pins:
//!   - bldimg: `docker.io/fnando/stellar-cli-experimental@sha256:85e76e…`.
//!     TODO: swap to `docker.io/stellar/stellar-cli@sha256:<…>` once
//!     `stellar/stellar-cli-docker` publishes a canonical tag matching the
//!     cli version under test.
//!   - source_repo + source_rev: a specific commit on
//!     `stellar/soroban-examples`. The `hello_world` contract there is the
//!     smallest, most-stable example; we build just that with `--package`.

use gix::progress::Discard;
use predicates::prelude::{predicate, PredicateBooleanExt};
use soroban_test::TestEnv;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

const PINNED_BLDIMG: &str =
    "docker.io/fnando/stellar-cli-experimental@sha256:85e76eae8bf9f47ba94391214b76f8fa2b9d7b28171774dfafaf5b8d613a74d3";
const PINNED_SOURCE_REPO: &str = "github:stellar/soroban-examples";
const PINNED_SOURCE_REV: &str = "7b168174ae1268dab91a0190d80a94ab7ff41b59";
/// `soroban-examples` has no root `Cargo.toml` — each example is its own
/// crate in a subdirectory. The cli's source-root resolver anchors the
/// bind-mount + the recorded bldopt to the clone root, so the manifest-path
/// stays portable as `hello_world/Cargo.toml` regardless of where the user
/// invoked from.
const PINNED_MANIFEST_PATH: &str = "hello_world/Cargo.toml";

/// Build a verifiable wasm for the pinned hello-world example and write it to
/// `<sandbox>/out/soroban_hello_world_contract.wasm`. Returns the on-disk path.
fn build_verifiable_hello_world(sandbox: &TestEnv) -> PathBuf {
    let out_dir = sandbox.dir().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    sandbox
        .new_assert_cmd("contract")
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg(PINNED_BLDIMG)
        .arg("--source-repo")
        .arg(PINNED_SOURCE_REPO)
        .arg("--source-rev")
        .arg(PINNED_SOURCE_REV)
        .arg("--manifest-path")
        .arg(PINNED_MANIFEST_PATH)
        .arg("--out-dir")
        .arg(&out_dir)
        .current_dir(prepared_source_tree(sandbox))
        .assert()
        .success();
    out_dir.join("soroban_hello_world_contract.wasm")
}

/// Materialize the pinned `stellar/soroban-examples` source tree at `<sandbox>/soroban-examples`
/// so the verifiable build has a workspace_root to bind-mount into the
/// container. The host's source tree is what the bldimg actually compiles;
/// `source_repo` + `source_rev` recorded into the wasm only tell a future
/// verifier where to fetch from. We clone via gix to stay shell-free.
fn prepared_source_tree(sandbox: &TestEnv) -> PathBuf {
    let dir = sandbox.dir().join("soroban-examples");
    if dir.exists() {
        return dir;
    }
    // Mirror what the cli's `verify::clone_git_source` does — same gix call
    // sequence, same flags — so the test exercises the production code path
    // a third-party verifier would hit.
    let interrupt = AtomicBool::new(false);
    let mut prepare = gix::prepare_clone_bare("https://github.com/stellar/soroban-examples", &dir)
        .expect("prepare_clone_bare");
    let (repo, _) = prepare.fetch_only(Discard, &interrupt).expect("fetch_only");
    let oid = gix::ObjectId::from_hex(PINNED_SOURCE_REV.as_bytes()).expect("rev hex");
    let object = repo.find_object(oid).expect("find_object");
    let commit = object.peel_to_commit().expect("peel_to_commit");
    let tree_id = commit.tree_id().expect("tree_id");
    let index = gix::index::State::from_tree(
        &tree_id,
        &repo.objects,
        gix::validate::path::component::Options::default(),
    )
    .expect("from_tree");
    let mut index_file = gix::index::File::from_state(index, dir.join(".git").join("index"));
    gix::worktree::state::checkout(
        &mut index_file,
        &dir,
        repo.objects.clone().into_arc().expect("into_arc"),
        &Discard,
        &Discard,
        &interrupt,
        gix::worktree::state::checkout::Options {
            destination_is_initially_empty: true,
            overwrite_existing: true,
            ..Default::default()
        },
    )
    .expect("checkout");
    dir
}

/// Happy path: build a verifiable wasm, then verify it from the local file.
/// Asserts the cli prints `Verified:` on stdout (or stderr; we accept either
/// via `predicates`).
#[test]
fn verify_wasm_succeeds_for_freshly_built_verifiable_wasm() {
    let sandbox = TestEnv::default();
    let wasm = build_verifiable_hello_world(&sandbox);

    sandbox
        .new_assert_cmd("contract")
        .arg("verify")
        .arg("--wasm")
        .arg(&wasm)
        .arg("--trust")
        .assert()
        .success()
        .stderr(predicate::str::contains("Verified:"));
}

/// Build verifiable → upload to local network → verify by --id. Exercises
/// the wasm::fetch_from_contract path through the verify command.
#[tokio::test]
async fn verify_id_succeeds_after_upload() {
    let sandbox = TestEnv::new();
    let wasm = build_verifiable_hello_world(&sandbox);
    let wasm_str = wasm.to_string_lossy().to_string();

    // Upload (cheaper than full deploy; verify only needs the wasm bytes, which
    // upload puts on-ledger under a known hash). `--id` accepts a contract id
    // OR an alias OR (via wasm_hash) any thing the network can resolve to wasm.
    // The deploy path is what gives us a contract id we can pass to --id.
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
        .arg("--trust")
        .assert()
        .success()
        .stderr(predicate::str::contains("Verified:"));
}

/// Flip a byte in a verifiable wasm and confirm `contract verify` reports the
/// mismatch (different hashes).
#[test]
fn verify_wasm_fails_on_tampered_bytes() {
    let sandbox = TestEnv::default();
    let wasm = build_verifiable_hello_world(&sandbox);

    // Tamper: corrupt a byte somewhere in the middle of the WASM. The custom
    // section that holds contractmetav0 is near the end; flipping a code byte
    // changes the bytes-under-comparison without invalidating the WASM enough
    // to break the cli's metadata parse.
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
        .arg("--trust")
        .assert()
        .failure()
        .stderr(predicate::str::contains("verification failed"));
}
