use core::fmt;

use bollard::{ClientVersion, Docker};
use clap::ValueEnum;
#[allow(unused_imports)]
// Need to add this for windows, since we are only using this crate for the unix fn try_docker_desktop_socket
use home::home_dir;

use crate::print;

pub const DOCKER_HOST_HELP: &str = "Optional argument to override the default docker host. This is useful when you are using a non-standard docker host path for your Docker-compatible container runtime, e.g. Docker Desktop defaults to $HOME/.docker/run/docker.sock instead of /var/run/docker.sock";

// DEFAULT_DOCKER_HOST is from the bollard crate on the main branch, which has not been released yet: https://github.com/fussybeaver/bollard/blob/0972b1aac0ad5c08798e100319ddd0d2ee010365/src/docker.rs#L64
#[cfg(unix)]
pub const DEFAULT_DOCKER_HOST: &str = "unix:///var/run/docker.sock";

#[cfg(windows)]
pub const DEFAULT_DOCKER_HOST: &str = "npipe:////./pipe/docker_engine";

// DEFAULT_TIMEOUT and API_DEFAULT_VERSION are from the bollard crate
const DEFAULT_TIMEOUT: u64 = 120;
const API_DEFAULT_VERSION: &ClientVersion = &ClientVersion {
    major_version: 1,
    minor_version: 40,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("⛔ ️Failed to start container: {0}")]
    BollardErr(#[from] bollard::errors::Error),

    #[error("URI scheme is not supported: {uri}")]
    UnsupportedURISchemeError { uri: String },
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

    #[allow(unused_variables)]
    pub(crate) async fn connect_to_docker(&self, printer: &print::Print) -> Result<Docker, Error> {
        // if no docker_host is provided, use the default docker host:
        // "unix:///var/run/docker.sock" on unix machines
        // "npipe:////./pipe/docker_engine" on windows machines
        let host = self.docker_host.as_ref().map_or_else(
            || DEFAULT_DOCKER_HOST.to_string(),
            std::string::ToString::to_string,
        );

        // this is based on the `connect_with_defaults` method which has not yet been released in the bollard crate
        // https://github.com/fussybeaver/bollard/blob/0972b1aac0ad5c08798e100319ddd0d2ee010365/src/docker.rs#L660
        let connection = match host.clone() {
            // if tcp or http, use connect_with_http_defaults
            // if unix and host starts with "unix://" use connect_with_unix
            // if windows and host starts with "npipe://", use connect_with_named_pipe
            // else default to connect_with_unix
            h if h.starts_with("tcp://") || h.starts_with("http://") => {
                Docker::connect_with_http_defaults()
            }
            #[cfg(unix)]
            h if h.starts_with("unix://") => {
                Docker::connect_with_unix(&h, DEFAULT_TIMEOUT, API_DEFAULT_VERSION)
            }
            #[cfg(windows)]
            h if h.starts_with("npipe://") => {
                Docker::connect_with_named_pipe(&h, DEFAULT_TIMEOUT, API_DEFAULT_VERSION)
            }
            _ => {
                return Err(Error::UnsupportedURISchemeError {
                    uri: host.to_string(),
                });
            }
        }?;

        match check_docker_connection(&connection).await {
            Ok(()) => Ok(connection),
            // If we aren't able to connect with the defaults, or with the provided docker_host
            // try to connect with the default docker desktop socket since that is a common use case for devs
            #[allow(unused_variables)]
            Err(e) => {
                // if on unix, try to connect to the default docker desktop socket
                #[cfg(unix)]
                {
                    let docker_desktop_connection = try_docker_desktop_socket(&host, printer)?;
                    match check_docker_connection(&docker_desktop_connection).await {
                        Ok(()) => Ok(docker_desktop_connection),
                        Err(err) => Err(err)?,
                    }
                }

                #[cfg(windows)]
                {
                    Err(e)?
                }
            }
        }
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
        self.0.to_string()
    }
}

#[cfg(unix)]
fn try_docker_desktop_socket(
    host: &str,
    printer: &print::Print,
) -> Result<Docker, bollard::errors::Error> {
    let default_docker_desktop_host =
        format!("{}/.docker/run/docker.sock", home_dir().unwrap().display());
    printer.warnln(format!("Failed to connect to Docker daemon at {host}."));

    printer.infoln(format!(
        "Attempting to connect to the default Docker Desktop socket at {default_docker_desktop_host} instead."
    ));

    Docker::connect_with_unix(
        &default_docker_desktop_host,
        DEFAULT_TIMEOUT,
        API_DEFAULT_VERSION,
    ).map_err(|e| {
        printer.errorln(format!(
            "Failed to connect to the Docker daemon at {host:?}. Is the docker daemon running?"
        ));
        printer.infoln(
            "Running a local Stellar network requires a Docker-compatible container runtime."
        );
        printer.infoln(
            "Please note that if you are using Docker Desktop, you may need to utilize the `--docker-host` flag to pass in the location of the docker socket on your machine."
        );
        e
    })
}

// When bollard is not able to connect to the docker daemon, it returns a generic ConnectionRefused error
// This method attempts to connect to the docker daemon and returns a more specific error message
async fn check_docker_connection(docker: &Docker) -> Result<(), bollard::errors::Error> {
    match docker.version().await {
        Ok(_version) => Ok(()),
        Err(err) => Err(err),
    }
}
