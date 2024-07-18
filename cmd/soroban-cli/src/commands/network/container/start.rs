use std::collections::HashMap;

use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
    service::{HostConfig, PortBinding},
};
use futures_util::TryStreamExt;

use crate::commands::network::container::shared::{Error as ConnectionError, Network};

use super::shared::{Args, Name};

const DEFAULT_PORT_MAPPING: &str = "8000:8000";
const DOCKER_IMAGE: &str = "docker.io/stellar/quickstart";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("⛔ ️Failed to connect to docker: {0}")]
    DockerConnectionFailed(#[from] ConnectionError),

    #[error("⛔ ️Failed to create container: {0}")]
    CreateContainerFailed(#[from] bollard::errors::Error),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: Args,

    /// Network to start
    pub network: Network,

    /// Optional argument to specify the container name
    #[arg(long)]
    pub name: Option<String>,

    /// Optional argument to specify the limits for the local network only
    #[arg(short = 'l', long)]
    pub limits: Option<String>,

    /// Argument to specify the `HOST_PORT:CONTAINER_PORT` mapping
    #[arg(short = 'p', long, num_args = 1.., default_value = DEFAULT_PORT_MAPPING)]
    pub ports_mapping: Vec<String>,

    /// Optional argument to override the default docker image tag for the given network
    #[arg(short = 't', long)]
    pub image_tag_override: Option<String>,

    /// Optional argument to specify the protocol version for the local network only
    #[arg(short = 'v', long)]
    pub protocol_version: Option<String>,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        println!("ℹ️  Starting {} network", &self.network);
        self.run_docker_command().await
    }

    async fn run_docker_command(&self) -> Result<(), Error> {
        let docker = self.container_args.connect_to_docker().await?;

        let image = self.get_image_name();
        docker
            .create_image(
                Some(CreateImageOptions {
                    from_image: image.clone(),
                    ..Default::default()
                }),
                None,
                None,
            )
            .try_collect::<Vec<_>>()
            .await?;

        let config = Config {
            image: Some(image),
            cmd: Some(self.get_container_args()),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            host_config: Some(HostConfig {
                auto_remove: Some(true),
                port_bindings: Some(self.get_port_mapping()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let create_container_response = docker
            .create_container(
                Some(CreateContainerOptions {
                    name: self.container_name().get_internal_container_name(),
                    ..Default::default()
                }),
                config,
            )
            .await?;

        docker
            .start_container(
                &create_container_response.id,
                None::<StartContainerOptions<String>>,
            )
            .await?;
        println!(
            "✅ Container started: {}",
            self.container_name().get_external_container_name()
        );
        self.print_log_message();
        self.print_stop_message();
        Ok(())
    }

    fn get_image_name(&self) -> String {
        // this can be overriden with the `-t` flag
        let mut image_tag = match &self.network {
            Network::Pubnet => "latest",
            Network::Futurenet => "future",
            _ => "testing", // default to testing for local and testnet
        };

        if let Some(image_override) = &self.image_tag_override {
            println!(
                "Overriding docker image tag to use '{image_override}' instead of '{image_tag}'"
            );
            image_tag = image_override;
        }

        format!("{DOCKER_IMAGE}:{image_tag}")
    }

    fn get_container_args(&self) -> Vec<String> {
        [
            format!("--{}", self.network),
            "--enable rpc,horizon".to_string(),
            self.get_protocol_version_arg(),
            self.get_limits_arg(),
        ]
        .iter()
        .filter(|&s| !s.is_empty())
        .cloned()
        .collect()
    }

    // The port mapping in the bollard crate is formatted differently than the docker CLI. In the docker CLI, we usually specify exposed ports as `-p  HOST_PORT:CONTAINER_PORT`. But with the bollard crate, it is expecting the port mapping to be a map of the container port (with the protocol) to the host port.
    fn get_port_mapping(&self) -> HashMap<String, Option<Vec<PortBinding>>> {
        let mut port_mapping_hash = HashMap::new();
        for port_mapping in &self.ports_mapping {
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

        port_mapping_hash
    }

    fn container_name(&self) -> Name {
        Name::new(self.name.clone().unwrap_or(self.network.to_string()))
    }

    fn print_log_message(&self) {
        let log_message = format!(
            "ℹ️ To see the logs for this container run: stellar network container logs {container_name} {additional_flags}",
            container_name = self.container_name().get_external_container_name(),
            additional_flags = self.container_args.get_additional_flags(),
        );
        println!("{log_message}");
    }

    fn print_stop_message(&self) {
        let stop_message =
            format!(
            "ℹ️ To stop this container run: stellar network container stop {container_name} {additional_flags}",
            container_name = self.container_name().get_external_container_name(),
            additional_flags = self.container_args.get_additional_flags(),
        );
        println!("{stop_message}");
    }

    fn get_protocol_version_arg(&self) -> String {
        if self.network == Network::Local && self.protocol_version.is_some() {
            let version = self.protocol_version.as_ref().unwrap();
            format!("--protocol-version {version}")
        } else {
            String::new()
        }
    }

    fn get_limits_arg(&self) -> String {
        if self.network == Network::Local && self.limits.is_some() {
            let limits = self.limits.as_ref().unwrap();
            format!("--limits {limits}")
        } else {
            String::new()
        }
    }
}
