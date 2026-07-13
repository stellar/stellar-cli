//! End-to-end tests for `stellar container start|stop|logs`.
//!
//! These never touch a real container runtime. Instead they drop a fake `docker`
//! / `container` executable into a temp dir, point `PATH` at it, and assert on
//! both the CLI's behavior and the exact argv the fake was invoked with. This
//! exercises the real spawn path and stderr classification that the unit tests in
//! `shared.rs` can't reach.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::predicate;
use soroban_test::TestEnv;

/// A sandbox with an isolated `PATH` that only contains the fake engines we
/// explicitly install, so the real `docker`/`container` on the host is never hit.
struct EngineSandbox {
    env: TestEnv,
    bin_dir: PathBuf,
    log: PathBuf,
}

impl EngineSandbox {
    fn new() -> Self {
        let env = TestEnv::default();
        let bin_dir = env.dir().join("fake-bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let log = env.dir().join("engine-invocations.log");
        Self { env, bin_dir, log }
    }

    /// Installs a fake engine binary named `name` (e.g. `docker` or `container`).
    /// Each invocation appends its argv to the shared log; behavior is driven by
    /// the `FAKE_ENGINE_MODE` env var (`ok`, `already_running`, `not_found`). The
    /// stderr wording matches the real engine the binary is impersonating so the
    /// CLI's per-engine classifiers are exercised end-to-end.
    fn install_engine(&self, name: &str) {
        // Real-world stderr strings the CLI classifies, per engine.
        let (already_running, not_found) = if name == "container" {
            (
                "Error: container with id stellar-local already exists",
                r#"Error: internalError: "failed to stop container" (cause: "notFound: "container with ID stellar-local not found"")"#,
            )
        } else {
            (
                r#"docker: Error response from daemon: Conflict. The container name "/stellar-local" is already in use."#,
                "Error response from daemon: No such container: stellar-local",
            )
        };
        let script = format!(
            r#"#!/bin/sh
echo "{name} $@" >> "{log}"
case " $* " in
  *" pull "*) exit 0 ;;
  *" run "*)
    case "$FAKE_ENGINE_MODE" in
      already_running)
        echo '{already_running}' >&2
        exit 1 ;;
      *) exit 0 ;;
    esac ;;
  *" stop "*)
    case "$FAKE_ENGINE_MODE" in
      not_found)
        echo '{not_found}' >&2
        exit 1 ;;
      *) exit 0 ;;
    esac ;;
  *) exit 0 ;;
esac
"#,
            name = name,
            log = self.log.display(),
        );
        let path = self.bin_dir.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// Builds a `container` subcommand whose `PATH` is only the fake bin dir.
    fn cmd(&self, mode: &str) -> Command {
        let mut cmd = self.env.new_assert_cmd("container");
        cmd.env("PATH", &self.bin_dir)
            .env("FAKE_ENGINE_MODE", mode)
            // Don't inherit a real DOCKER_HOST/engine from the developer's shell.
            .env_remove("DOCKER_HOST")
            .env_remove("STELLAR_CONTAINER_ENGINE");
        cmd
    }

    fn invocations(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

fn line_for(log: &str, needle: &str) -> String {
    log.lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no invocation containing {needle:?} in:\n{log}"))
        .to_string()
}

#[test]
fn start_defaults_to_docker_and_passes_expected_args() {
    let s = EngineSandbox::new();
    s.install_engine("docker");

    s.cmd("ok")
        .args(["start", "local"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Started container"));

    let log = s.invocations();
    // Pulled and ran via docker (not the apple `image pull` form).
    assert!(line_for(&log, "pull").starts_with("docker pull "));
    let run = line_for(&log, " run ");
    assert!(run.starts_with("docker "), "expected docker binary: {run}");
    assert!(run.contains("--name stellar-local"), "run line: {run}");
    assert!(run.contains("-p 8000:8000"), "run line: {run}");
    // No host override unless requested.
    assert!(!run.contains("-H "), "unexpected -H: {run}");
}

#[test]
fn start_passes_docker_host_as_h_flag() {
    let s = EngineSandbox::new();
    s.install_engine("docker");

    s.cmd("ok")
        .args(["start", "local", "--docker-host", "ssh://me@host"])
        .assert()
        .success();

    let run = line_for(&s.invocations(), " run ");
    assert!(run.contains("-H ssh://me@host"), "run line: {run}");
}

#[test]
fn start_reports_already_running_from_stderr() {
    let s = EngineSandbox::new();
    s.install_engine("docker");

    s.cmd("already_running")
        .args(["start", "local"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already running"));
}

#[test]
fn stop_success() {
    let s = EngineSandbox::new();
    s.install_engine("docker");

    s.cmd("ok")
        .args(["stop", "local"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Container stopped"));

    assert!(line_for(&s.invocations(), " stop ").contains("stop stellar-local"));
}

#[test]
fn stop_reports_not_found_from_stderr() {
    let s = EngineSandbox::new();
    s.install_engine("docker");

    s.cmd("not_found")
        .args(["stop", "local"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("container local not found"));
}

#[test]
fn apple_engine_uses_container_binary_and_image_pull() {
    let s = EngineSandbox::new();
    // Only the apple engine exists on PATH; if the CLI shelled out to docker it
    // would fail with a not-found error instead.
    s.install_engine("container");

    s.cmd("ok")
        .args(["start", "local", "--engine", "apple-container"])
        .assert()
        .success();

    let log = s.invocations();
    // Apple groups image ops under `image` (singular) and takes no `--tail`.
    assert!(line_for(&log, "pull").starts_with("container image pull "));
    let run = line_for(&log, " run ");
    assert!(
        run.starts_with("container "),
        "expected container binary: {run}"
    );
}

#[test]
fn apple_engine_reports_already_running_from_stderr() {
    let s = EngineSandbox::new();
    s.install_engine("container");

    // The fake `container` emits Apple's `already exists` wording, so this only
    // passes if the CLI classifies stderr with the apple-container matcher.
    s.cmd("already_running")
        .args(["start", "local", "--engine", "apple-container"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already running"));
}

#[test]
fn apple_engine_reports_not_found_from_stderr() {
    let s = EngineSandbox::new();
    s.install_engine("container");

    // The fake `container` emits Apple's `not found` wording here.
    s.cmd("not_found")
        .args(["stop", "local", "--engine", "apple-container"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("container local not found"));
}

#[test]
fn apple_engine_selectable_via_env_var() {
    let s = EngineSandbox::new();
    s.install_engine("container");

    s.cmd("ok")
        .env("STELLAR_CONTAINER_ENGINE", "apple-container")
        .args(["start", "local"])
        .assert()
        .success();

    assert!(line_for(&s.invocations(), " run ").starts_with("container "));
}

#[test]
fn warns_and_drops_docker_host_for_apple_engine() {
    let s = EngineSandbox::new();
    s.install_engine("container");

    s.cmd("ok")
        .args([
            "start",
            "local",
            "--engine",
            "apple-container",
            "--docker-host",
            "ssh://me@host",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "is ignored because the `apple-container` engine",
        ));

    // The ignored host must not reach the container binary.
    assert!(
        !s.invocations().contains("-H "),
        "host leaked to container CLI"
    );
}

#[test]
fn errors_when_engine_binary_is_missing() {
    // No engine installed on the isolated PATH.
    let s = EngineSandbox::new();

    s.cmd("ok")
        .args(["stop", "local"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "is docker installed and on your PATH?",
        ));
}
