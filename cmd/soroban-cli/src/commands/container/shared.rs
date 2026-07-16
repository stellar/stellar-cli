use core::fmt;

use clap::ValueEnum;
use tokio::process::Command;

use crate::print::Print;

pub const DOCKER_HOST_HELP: &str = "Optional argument to override the default docker host. This is useful when you are using a non-standard docker host path for your Docker-compatible container runtime, e.g. Docker Desktop defaults to $HOME/.docker/run/docker.sock instead of /var/run/docker.sock";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to run {program}: {source}; is {program} installed and on your PATH?")]
    NotFound {
        program: String,
        source: std::io::Error,
    },

    #[error("failed to run {program}: {source}")]
    Command {
        program: String,
        source: std::io::Error,
    },
}

/// Container runtime to shell out to.
#[derive(ValueEnum, Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Engine {
    /// Docker, or any Docker-compatible CLI.
    #[default]
    Docker,
    /// Apple's `container` CLI (macOS 26+, Apple silicon).
    AppleContainer,
}

impl Engine {
    /// The engine used when no explicit `--engine` flag is passed: honors the
    /// `STELLAR_CONTAINER_ENGINE` env var, otherwise docker. An explicit
    /// `--engine` still overrides this (clap injects it into `Args::engine`);
    /// this is for callers without CLI args, e.g. `stellar doctor`.
    pub(crate) fn resolved_default() -> Engine {
        std::env::var("STELLAR_CONTAINER_ENGINE")
            .ok()
            .and_then(|value| Engine::from_str(&value, true).ok())
            .unwrap_or_default()
    }

    /// Whether `STELLAR_CONTAINER_ENGINE`, if set, names a known engine. An
    /// unset var is valid (it selects the docker default). `resolved_default`
    /// silently falls back to docker on a bad value, so `stellar doctor` probes
    /// this first to surface the typo instead of masking it.
    pub(crate) fn is_valid_engine() -> bool {
        std::env::var("STELLAR_CONTAINER_ENGINE")
            .ok()
            .is_none_or(|value| Engine::from_str(&value, true).is_ok())
    }

    /// The engine flag values, comma-separated, for help and error text. Kept
    /// in sync with the enum via `ValueEnum` rather than hardcoded.
    pub(crate) fn supported_engines() -> String {
        Engine::value_variants()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The executable name to invoke on `PATH`.
    pub(crate) fn program(self) -> &'static str {
        match self {
            Engine::Docker => "docker",
            Engine::AppleContainer => "container",
        }
    }

    /// Only docker honors `--docker-host`/`DOCKER_HOST`.
    fn supports_docker_host(self) -> bool {
        matches!(self, Engine::Docker)
    }

    // The stderr-classification helpers below are the single place per-engine
    // wording lives.
    fn is_container_already_running(self, stderr: &str) -> bool {
        match self {
            Engine::Docker => stderr.contains("already in use"),
            // Apple emits e.g. `Error: container with id stellar-local already exists`.
            Engine::AppleContainer => stderr.contains("already exists"),
        }
    }

    fn is_container_not_found(self, stderr: &str) -> bool {
        match self {
            Engine::Docker => stderr.contains("No such container"),
            // Apple emits e.g. `... notFound: "container with ID stellar-local not found"`.
            Engine::AppleContainer => stderr.contains("not found"),
        }
    }
}

impl fmt::Display for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Emit the flag value, which differs from the binary name (`program()`).
        let variant_str = match self {
            Engine::Docker => "docker",
            Engine::AppleContainer => "apple-container",
        };

        write!(f, "{variant_str}")
    }
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Args {
    /// Optional argument to override the default docker host. This is useful when you are using a non-standard docker host path for your Docker-compatible container runtime, e.g. Docker Desktop defaults to $HOME/.docker/run/docker.sock instead of /var/run/docker.sock
    #[arg(short = 'd', long, help = DOCKER_HOST_HELP, env = "DOCKER_HOST")]
    pub docker_host: Option<String>,

    /// Container engine to use [default: docker].
    #[arg(long, value_enum, env = "STELLAR_CONTAINER_ENGINE")]
    pub engine: Option<Engine>,
}

impl Args {
    /// Resolves the effective engine. This is the single seam where a future
    /// `stellar container use <engine>` config default (flag/env > config > docker)
    /// can slot in.
    fn engine(&self) -> Engine {
        self.engine.unwrap_or_default()
    }

    pub(crate) fn get_additional_flags(&self) -> String {
        let engine = self.engine();
        let mut parts = Vec::new();

        if engine != Engine::default() {
            parts.push(format!("--engine {engine}"));
        }

        if engine.supports_docker_host() {
            if let Some(docker_host) = &self.docker_host {
                parts.push(format!("--docker-host {docker_host}"));
            }
        }

        parts.join(" ")
    }

    /// Warns when `--docker-host`/`DOCKER_HOST` was provided but the selected
    /// engine ignores it.
    pub(crate) fn warn_if_host_ignored(&self, print: &Print) {
        if let Some(message) = self.host_ignored_warning() {
            print.warnln(message);
        }
    }

    fn host_ignored_warning(&self) -> Option<String> {
        if self.docker_host.is_some() && !self.engine().supports_docker_host() {
            Some(format!(
                "`--docker-host`/`DOCKER_HOST` is ignored because the `{}` engine does not support it",
                self.engine()
            ))
        } else {
            None
        }
    }

    /// Maps a spawn/IO error to an engine-aware error carrying the binary name.
    pub(crate) fn io_error(&self, err: std::io::Error) -> Error {
        let program = self.engine().program().to_string();
        if err.kind() == std::io::ErrorKind::NotFound {
            Error::NotFound {
                program,
                source: err,
            }
        } else {
            Error::Command {
                program,
                source: err,
            }
        }
    }

    pub(crate) fn is_container_already_running(&self, stderr: &str) -> bool {
        self.engine().is_container_already_running(stderr)
    }

    pub(crate) fn is_container_not_found(&self, stderr: &str) -> bool {
        self.engine().is_container_not_found(stderr)
    }

    /// Builds the base command for the selected engine. For docker, a
    /// `--docker-host` (or `DOCKER_HOST` env) value is passed as `-H <host>`; the
    /// `-H` flag outranks `DOCKER_CONTEXT`, so the override is honored even when a
    /// docker context is active. Host resolution is otherwise left to the CLI.
    fn base_command(&self) -> Command {
        let engine = self.engine();
        let mut cmd = Command::new(engine.program());
        if engine.supports_docker_host() {
            if let Some(host) = &self.docker_host {
                cmd.args(["-H", host]);
            }
        }
        cmd
    }

    pub(crate) fn pull_command(&self, image: &str) -> Command {
        let mut cmd = self.base_command();
        match self.engine() {
            Engine::Docker => cmd.args(["pull", image]),
            // Apple's CLI groups image operations under the `image` subcommand.
            Engine::AppleContainer => cmd.args(["image", "pull", image]),
        };
        cmd
    }

    pub(crate) fn run_command(&self, name: &str, ports: &[String]) -> Command {
        let mut cmd = self.base_command();
        cmd.args(["run", "-d", "--rm", "--name", name]);
        for port in ports {
            cmd.args(["-p", port]);
        }
        cmd
    }

    pub(crate) fn stop_command(&self, name: &str) -> Command {
        let mut cmd = self.base_command();
        cmd.args(["stop", name]);
        cmd
    }

    pub(crate) fn logs_command(&self, name: &str) -> Command {
        let mut cmd = self.base_command();
        match self.engine() {
            Engine::Docker => cmd.args(["logs", "-f", "--tail", "all", name]),
            // `--tail all` is docker-specific; Apple's `container logs` omits it.
            Engine::AppleContainer => cmd.args(["logs", "-f", name]),
        };
        cmd
    }
}

/// Resource limits for commands that *run* a container (e.g. `container start`).
/// Kept separate from [`Args`] so commands like `stop`/`logs`, which only act on
/// an existing container, don't advertise flags they ignore. Both `--cpus` and
/// `--memory` are accepted verbatim by docker and Apple's `container` on `run`,
/// so the values pass through unparsed.
#[derive(Debug, clap::Parser, Clone, Default)]
pub struct RunArgs {
    /// Limit the number of CPUs available to the container, e.g. `2`. A whole
    /// number: Apple's `container` engine does not accept fractional CPUs.
    #[arg(long)]
    pub cpus: Option<u32>,

    /// Limit the memory available to the container, e.g. `2g` or `512m`.
    #[arg(long)]
    pub memory: Option<String>,
}

impl RunArgs {
    /// The resource-limit flags as `run` argv tokens (`--cpus <n>`,
    /// `--memory <size>`); empty when no limit is set.
    pub(crate) fn flags(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(cpus) = &self.cpus {
            out.push("--cpus".to_string());
            out.push(cpus.to_string());
        }
        if let Some(memory) = &self.memory {
            out.push("--memory".to_string());
            out.push(memory.clone());
        }
        out
    }

    /// Append the resource-limit flags to a `run` command. A no-op when unset.
    pub(crate) fn apply(&self, cmd: &mut Command) {
        cmd.args(self.flags());
    }
}

#[derive(ValueEnum, Debug, Copy, Clone, PartialEq)]
pub enum Network {
    Local,
    Testnet,
    Futurenet,
    Pubnet,
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant_str = match self {
            Network::Local => "local",
            Network::Testnet => "testnet",
            Network::Futurenet => "futurenet",
            Network::Pubnet => "pubnet",
        };

        write!(f, "{variant_str}")
    }
}

pub struct Name(pub String);
impl Name {
    pub fn get_internal_container_name(&self) -> String {
        format!("stellar-{}", self.0)
    }

    pub fn get_external_container_name(&self) -> String {
        self.0.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::with_env_guard;
    use serial_test::serial;

    fn args(docker_host: Option<&str>, engine: Option<Engine>) -> Args {
        Args {
            docker_host: docker_host.map(String::from),
            engine,
        }
    }

    fn program_of(cmd: &Command) -> String {
        cmd.as_std().get_program().to_string_lossy().into_owned()
    }

    fn args_of(cmd: &Command) -> Vec<String> {
        cmd.as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn supported_engines_lists_flag_values() {
        assert_eq!(Engine::supported_engines(), "docker, apple-container");
    }

    #[test]
    #[serial]
    fn is_valid_engine_tracks_env_var() {
        const KEY: &str = "STELLAR_CONTAINER_ENGINE";

        with_env_guard(&[KEY], || {
            // An unset var is valid: it selects the docker default.
            assert!(Engine::is_valid_engine());

            std::env::set_var(KEY, "apple-container");
            assert!(Engine::is_valid_engine());

            std::env::set_var(KEY, "podman");
            assert!(!Engine::is_valid_engine());
        });
    }

    #[test]
    fn engine_defaults_to_docker() {
        assert_eq!(args(None, None).engine(), Engine::Docker);
        assert_eq!(
            args(None, Some(Engine::AppleContainer)).engine(),
            Engine::AppleContainer
        );
    }

    #[test]
    fn docker_pull_uses_bare_pull() {
        let cmd = args(None, None).pull_command("img:tag");
        assert_eq!(program_of(&cmd), "docker");
        assert_eq!(args_of(&cmd), ["pull", "img:tag"]);
    }

    #[test]
    fn apple_pull_uses_image_pull_and_ignores_host() {
        let cmd = args(Some("ssh://host"), Some(Engine::AppleContainer)).pull_command("img:tag");
        assert_eq!(program_of(&cmd), "container");
        assert_eq!(args_of(&cmd), ["image", "pull", "img:tag"]);
    }

    #[test]
    fn docker_run_passes_host_as_h_flag() {
        let cmd =
            args(Some("ssh://host"), None).run_command("stellar-local", &["8000:8000".to_string()]);
        assert_eq!(program_of(&cmd), "docker");
        assert_eq!(
            args_of(&cmd),
            [
                "-H",
                "ssh://host",
                "run",
                "-d",
                "--rm",
                "--name",
                "stellar-local",
                "-p",
                "8000:8000"
            ]
        );
    }

    #[test]
    fn apple_run_omits_host() {
        let cmd = args(Some("ssh://host"), Some(Engine::AppleContainer))
            .run_command("stellar-local", &[]);
        assert_eq!(program_of(&cmd), "container");
        assert_eq!(
            args_of(&cmd),
            ["run", "-d", "--rm", "--name", "stellar-local"]
        );
    }

    #[test]
    fn docker_logs_include_tail_all_apple_omits_it() {
        assert_eq!(
            args_of(&args(None, None).logs_command("stellar-local")),
            ["logs", "-f", "--tail", "all", "stellar-local"]
        );
        assert_eq!(
            args_of(&args(None, Some(Engine::AppleContainer)).logs_command("stellar-local")),
            ["logs", "-f", "stellar-local"]
        );
    }

    #[test]
    fn additional_flags_carry_engine_and_host() {
        assert_eq!(args(None, None).get_additional_flags(), "");
        assert_eq!(
            args(None, Some(Engine::AppleContainer)).get_additional_flags(),
            "--engine apple-container"
        );
        assert_eq!(
            args(Some("ssh://host"), None).get_additional_flags(),
            "--docker-host ssh://host"
        );
        // Apple ignores the host, so it must not be suggested for follow-up commands.
        assert_eq!(
            args(Some("ssh://host"), Some(Engine::AppleContainer)).get_additional_flags(),
            "--engine apple-container"
        );
    }

    #[test]
    fn host_ignored_warning_only_for_non_docker_engines() {
        assert!(args(Some("ssh://host"), Some(Engine::AppleContainer))
            .host_ignored_warning()
            .is_some());
        assert!(args(Some("ssh://host"), None)
            .host_ignored_warning()
            .is_none());
        assert!(args(None, Some(Engine::AppleContainer))
            .host_ignored_warning()
            .is_none());
    }

    #[test]
    fn io_error_is_engine_aware() {
        let not_found = std::io::Error::from(std::io::ErrorKind::NotFound);
        match args(None, Some(Engine::AppleContainer)).io_error(not_found) {
            Error::NotFound { program, .. } => assert_eq!(program, "container"),
            Error::Command { .. } => panic!("expected NotFound, got Command"),
        }
    }

    #[test]
    fn docker_stderr_classifiers_match_expected_strings() {
        let docker = args(None, None);
        assert!(docker.is_container_already_running(
            r#"docker: Error response from daemon: Conflict. The container name "/stellar-local" is already in use"#
        ));
        assert!(!docker.is_container_already_running("some unrelated failure"));
        assert!(docker.is_container_not_found(
            "Error response from daemon: No such container: stellar-local"
        ));
        assert!(!docker.is_container_not_found("some unrelated failure"));
    }

    #[test]
    fn apple_stderr_classifiers_match_expected_strings() {
        let apple = args(None, Some(Engine::AppleContainer));
        assert!(apple
            .is_container_already_running("Error: container with id stellar-local already exists"));
        assert!(!apple.is_container_already_running("some unrelated failure"));
        assert!(apple.is_container_not_found(
            r#"Error: internalError: "failed to stop container" (cause: "notFound: "container with ID stellar-local not found"")"#
        ));
        assert!(!apple.is_container_not_found("some unrelated failure"));
    }

    #[test]
    fn run_args_flags_emit_only_set_limits() {
        assert!(RunArgs::default().flags().is_empty());
        assert_eq!(
            RunArgs {
                cpus: Some(1),
                memory: None,
            }
            .flags(),
            ["--cpus", "1"]
        );
        assert_eq!(
            RunArgs {
                cpus: Some(2),
                memory: Some("2g".to_string()),
            }
            .flags(),
            ["--cpus", "2", "--memory", "2g"]
        );
    }
}
