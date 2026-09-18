//! End-to-end coverage for `stellar contract build --verifiable`.
//!
//! Unlike the unit/validation tests, this drives a real container build, so it
//! requires a working docker/engine — hence it lives in the opt-in `integration`
//! suite (run with `--features it`). It asserts the happy path the cheaper tests
//! can't: a successful in-container build, the artifact copied back to the host
//! target dir, and the SEP-58 provenance meta stamped into the wasm.

use fs_extra::dir::CopyOptions;
use soroban_test::{AssertExt, TestEnv};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A digest-pinned ref for the official CLI image, resolved dynamically (pulling
/// first) so it never goes stale. `--verifiable` requires a digest pin with an
/// explicit registry host.
fn pinned_cli_image() -> String {
    let tag = "stellar/stellar-cli:latest";
    let pulled = Command::new("docker")
        .args(["pull", tag])
        .status()
        .expect("docker must be available for verifiable-build integration tests");
    assert!(pulled.success(), "failed to pull {tag}");

    let out = Command::new("docker")
        .args(["inspect", "--format", "{{index .RepoDigests 0}}", tag])
        .output()
        .expect("docker inspect");
    let repo_digest = String::from_utf8(out.stdout).unwrap().trim().to_string();
    // e.g. `stellar/stellar-cli@sha256:…`; prepend the registry host the CLI wants.
    format!("docker.io/{repo_digest}")
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed");
}

#[test]
fn verifiable_build_stamps_sep58_metadata_and_copies_artifact() {
    let sandbox = TestEnv::default();

    // Copy the workspace fixture into the sandbox and make it a clean git repo —
    // a verifiable build requires a committed tree.
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = cargo_dir.join("tests/fixtures/workspace");
    fs_extra::dir::copy(&fixture, sandbox.dir(), &CopyOptions::new()).unwrap();
    let workspace = sandbox.dir().join("workspace");
    git(&workspace, &["init", "-q", "-b", "main"]);
    // Never sign, regardless of the runner's global git config.
    git(&workspace, &["config", "commit.gpgsign", "false"]);
    git(&workspace, &["add", "-A"]);
    git(&workspace, &["commit", "-q", "-m", "init"]);

    let image = pinned_cli_image();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&workspace)
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg(&image)
        .arg("--package")
        .arg("add")
        // `--source-uri` needs only `--verifiable`; the hash is computed and stamped.
        .arg("--source-uri")
        .arg("https://example.com/src.tar.gz")
        .assert()
        .success();

    // The build runs in an extracted tempdir, so the artifact must be copied back
    // to the host workspace target dir.
    let wasm = workspace.join("target/wasm32v1-none/release/add.wasm");
    assert!(
        wasm.exists(),
        "verifiable build should copy the wasm to the host target dir"
    );

    let meta = sandbox
        .new_assert_cmd("contract")
        .current_dir(&workspace)
        .arg("info")
        .arg("meta")
        .arg("--wasm")
        .arg(&wasm)
        .assert()
        .success()
        .stdout_as_str();

    assert!(
        meta.contains("bldimg: docker.io/stellar/stellar-cli@sha256:"),
        "expected bldimg in meta, got:\n{meta}"
    );
    assert!(
        meta.contains("source_uri: https://example.com/src.tar.gz"),
        "expected source_uri in meta, got:\n{meta}"
    );
    assert!(
        meta.contains("source_sha256: "),
        "expected computed source_sha256 in meta, got:\n{meta}"
    );
    // bldopts are recorded verbatim (SEP-58), including the implied `--locked`.
    assert!(
        meta.contains("bldopt: --locked"),
        "expected implied --locked bldopt, got:\n{meta}"
    );
    assert!(
        meta.contains("bldopt: --package=add"),
        "expected --package bldopt, got:\n{meta}"
    );
}
