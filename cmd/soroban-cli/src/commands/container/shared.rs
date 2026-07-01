use core::fmt;

use clap::ValueEnum;
use tokio::process::Command;

pub const DOCKER_HOST_HELP: &str = "Optional argument to override the default docker host. This is useful when you are using a non-standard docker host path for your Docker-compatible container runtime, e.g. Docker Desktop defaults to $HOME/.docker/run/docker.sock instead of /var/run/docker.sock";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to run docker: {0}; is docker installed and on your PATH?")]
    DockerNotFound(std::io::Error),

    #[error("failed to run docker: {0}")]
    DockerCommand(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        if err.kind() == std::io::ErrorKind::NotFound {
            Error::DockerNotFound(err)
        } else {
            Error::DockerCommand(err)
        }
    }
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Args {
    /// Optional argument to override the default docker host. This is useful when you are using a non-standard docker host path for your Docker-compatible container runtime, e.g. Docker Desktop defaults to $HOME/.docker/run/docker.sock instead of /var/run/docker.sock
    #[arg(short = 'd', long, help = DOCKER_HOST_HELP, env = "DOCKER_HOST")]
    pub docker_host: Option<String>,
}

impl Args {
    pub(crate) fn get_additional_flags(&self) -> String {
        self.docker_host
            .as_ref()
            .map(|docker_host| format!("--docker-host {docker_host}"))
            .unwrap_or_default()
    }

    /// Builds a `docker` command, passing a `-H <host>` override when a `--docker-host` (or
    /// `DOCKER_HOST` env) value is provided. The `-H` flag outranks `DOCKER_CONTEXT`, so the
    /// override is honored even when a docker context is active. Host resolution is otherwise
    /// left to the docker CLI itself.
    pub(crate) fn docker_command(&self) -> Command {
        let mut cmd = Command::new("docker");
        if let Some(host) = &self.docker_host {
            cmd.args(["-H", host]);
        }
        cmd
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
