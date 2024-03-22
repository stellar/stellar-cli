use std::collections::HashMap;

use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
    service::{HostConfig, PortBinding},
    ClientVersion, Docker,
};
use futures_util::TryStreamExt;

#[allow(unused_imports)]
// Need to add this for windows, since we are only using this crate for the unix fn try_docker_desktop_socket
use home::home_dir;

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

pub struct DockerConnection {
    docker: Docker,
}

impl DockerConnection {
    pub async fn new() -> Self {
        DockerConnection {
            docker: connect_to_docker(&Some(DEFAULT_DOCKER_HOST.to_owned()))
                .await
                .unwrap(),
        }
    }

    async fn get_image_with_defaults(&self, image_name: &str) -> Result<(), Error> {
        self.docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: image_name.to_string(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await?;

        Ok(())
    }

    async fn get_container_with_defaults(&self, image_name: &str) -> Result<String, Error> {
        let default_port_mappings = vec!["8000:8000", "8001:8001"];
        // The port mapping in the bollard crate is formatted differently than the docker CLI. In the docker CLI, we usually specify exposed ports as `-p  HOST_PORT:CONTAINER_PORT`. But with the bollard crate, it is expecting the port mapping to be a map of the container port (with the protocol) to the host port.
        let mut port_mapping_hash = HashMap::new();
        for port_mapping in default_port_mappings {
            let ports_vec: Vec<&str> = port_mapping.split(':').collect();
            let from_port = ports_vec[0];
            let to_port = ports_vec[1];

            port_mapping_hash.insert(
                format!("{to_port}/tcp"),
                Some(vec![PortBinding {
                    host_ip: None,
                    host_port: Some(from_port.to_string()),
                }]),
            );
        }

        let config = Config {
            image: Some(image_name),
            cmd: None,
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            host_config: Some(HostConfig {
                auto_remove: Some(true),
                port_bindings: Some(port_mapping_hash),
                ..Default::default()
            }),
            ..Default::default()
        };

        let create_container_response = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: "FIX ME",
                    ..Default::default()
                }),
                config,
            )
            .await?;

        Ok(create_container_response.id)
    }

    async fn start_container_with_defaults(
        &self,
        container_response_id: &str,
    ) -> Result<(), bollard::errors::Error> { // deal with this error
        self.docker
            .start_container(container_response_id, None::<StartContainerOptions<String>>)
            .await
    }
}

pub async fn connect_to_docker(docker_host: &Option<String>) -> Result<Docker, Error> {
    // if no docker_host is provided, use the default docker host:
    // "unix:///var/run/docker.sock" on unix machines
    // "npipe:////./pipe/docker_engine" on windows machines

    let host = docker_host
        .clone()
        .unwrap_or(DEFAULT_DOCKER_HOST.to_string());

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
                let docker_desktop_connection = try_docker_desktop_socket(&host)?;
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

#[cfg(unix)]
fn try_docker_desktop_socket(host: &str) -> Result<Docker, bollard::errors::Error> {
    let default_docker_desktop_host =
        format!("{}/.docker/run/docker.sock", home_dir().unwrap().display());
    println!("Failed to connect to DOCKER_HOST: {host}.\nTrying to connect to the default Docker Desktop socket at {default_docker_desktop_host}.");

    Docker::connect_with_unix(
        &default_docker_desktop_host,
        DEFAULT_TIMEOUT,
        API_DEFAULT_VERSION,
    )
}

// When bollard is not able to connect to the docker daemon, it returns a generic ConnectionRefused error
// This method attempts to connect to the docker daemon and returns a more specific error message
async fn check_docker_connection(docker: &Docker) -> Result<(), bollard::errors::Error> {
    // This is a bit hacky, but the `client_addr` field is not directly accessible from the `Docker` struct, but we can access it from the debug string representation of the `Docker` struct
    let docker_debug_string = format!("{docker:#?}");
    let start_of_client_addr = docker_debug_string.find("client_addr: ").unwrap();
    let end_of_client_addr = docker_debug_string[start_of_client_addr..]
        .find(',')
        .unwrap();
    // Extract the substring containing the value of client_addr
    let client_addr = &docker_debug_string
        [start_of_client_addr + "client_addr: ".len()..start_of_client_addr + end_of_client_addr]
        .trim()
        .trim_matches('"');

    match docker.version().await {
        Ok(_version) => Ok(()),
        Err(err) => {
            println!(
                "⛔️ Failed to connect to the Docker daemon at {client_addr:?}. Is the docker daemon running?\nℹ️  Running a local Stellar network requires a Docker-compatible container runtime.\nℹ️  Please note that if you are using Docker Desktop, you may need to utilize the `--docker-host` flag to pass in the location of the docker socket on your machine.\n"
            );
            Err(err)
        }
    }
}
