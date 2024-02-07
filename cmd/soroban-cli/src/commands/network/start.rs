use bollard::{
    container::{Config, CreateContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
    service::{HostConfig, PortBinding},
    ClientVersion, Docker,
};

use futures_util::TryStreamExt;
use std::collections::HashMap;

#[derive(thiserror::Error, Debug)]
pub enum Error {}

const FROM_PORT: i32 = 8000;
const TO_PORT: i32 = 8000;
const CONTAINER_NAME: &str = "stellar";
const DOCKER_IMAGE: &str = "docker.io/stellar/quickstart";

// DEFAULT_TIMEOUT and API_DEFAULT_VERSION are from the bollard crate
const DEFAULT_TIMEOUT: u64 = 120;
const API_DEFAULT_VERSION: &ClientVersion = &ClientVersion {
    major_version: 1,
    minor_version: 40,
};

/// This command allows for starting a stellar quickstart container. To run it, you can use the following command:
/// `soroban network start <NETWORK> [OPTIONS] -- [DOCKER_RUN_ARGS]`
///
/// OPTIONS: refer to the options that are available to the quickstart image:
/// --enable-soroban-rpc - is enabled by default
/// --protocol-version (only for local network)
/// --limits (only for local network)

/// `DOCKER_RUN_ARGS`: These are arguments to be passed to the `docker run` command itself, and should be passed in after the slop `--`. Some common options are:
/// -p <`FROM_PORT`>:<`TO_PORT`> - this maps the port from the container to the host machine. By default, the port is 8000.
/// -d - this runs the container in detached mode, so that it runs in the background

// By default, without any optional arguments, the following docker command will run:
// docker run --rm -p 8000:8000 --name stellar stellar/quickstart:testing --testnet --enable-soroban-rpc

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// Network to start, e.g. local, testnet, futurenet, pubnet
    pub network: String,

    /// optional argument to override the default docker image tag for the given network
    #[arg(short = 't', long)]
    pub image_tag_override: Option<String>,

    /// optional argument to turn off soroban rpc
    #[arg(short = 'r', long)]
    pub disable_soroban_rpc: bool,

    /// optional argument to specify the protocol version for the local network only
    #[arg(short = 'p', long)]
    pub protocol_version: Option<String>,

    /// optional argument to specify the limits for the local network only
    #[arg(short = 'l', long)]
    pub limit: Option<String>,

    /// optional argument to override the default docker socket path
    #[arg(short = 'd', long)]
    pub docker_socket_path: Option<String>,

    /// optional arguments to pass to the docker run command
    #[arg(last = true, id = "DOCKER_RUN_ARGS")]
    pub slop: Vec<String>,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        println!("Starting {} network", &self.network);
        run_docker_command(self).await;
        Ok(())
    }
}

async fn run_docker_command(cmd: &Cmd) {
    let docker = connect_to_docker(cmd);

    let image = get_image_name(cmd);
    let create_image_options = Some(CreateImageOptions {
        from_image: image.clone(),
        ..Default::default()
    });

    docker
        .create_image(create_image_options, None, None)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    let container_args = get_container_args(cmd);
    let container_name = get_container_name(cmd);
    let port_mapping = get_port_mapping(cmd);

    let config = Config {
        image: Some(image),
        cmd: Some(container_args),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        host_config: Some(HostConfig {
            auto_remove: Some(true),
            port_bindings: Some(port_mapping),
            ..Default::default()
        }),
        ..Default::default()
    };

    let options = Some(CreateContainerOptions {
        name: container_name,
        platform: None,
    });

    let response = docker.create_container(options, config).await.unwrap();
    let _container = docker
        .start_container(&response.id, None::<StartContainerOptions<String>>)
        .await;
}

fn connect_to_docker(cmd: &Cmd) -> Docker {
    if cmd.docker_socket_path.is_some() {
        let socket = cmd.docker_socket_path.as_ref().unwrap();
        Docker::connect_with_socket(socket, DEFAULT_TIMEOUT, API_DEFAULT_VERSION).unwrap()
    } else {
        Docker::connect_with_socket_defaults().unwrap()
    }
}

fn get_container_args(cmd: &Cmd) -> Vec<String> {
    let enable_soroban_rpc = if cmd.disable_soroban_rpc {
        String::new()
    } else {
        "--enable-soroban-rpc".to_string()
    };

    [
        format!("--{}", cmd.network),
        enable_soroban_rpc,
        get_protocol_version_arg(cmd),
        get_limits_arg(cmd),
    ]
    .iter()
    .filter(|&s| !s.is_empty())
    .cloned()
    .collect()
}

fn get_image_name(cmd: &Cmd) -> String {
    // this can be overriden with the `-t` flag
    let mut image_tag = match cmd.network.as_str() {
        "testnet" => "testing",
        "futurenet" => "soroban-dev",
        _ => "latest", // default to latest for local and pubnet
    };

    if cmd.image_tag_override.is_some() {
        let override_tag = cmd.image_tag_override.as_ref().unwrap();
        println!("Overriding docker image tag to use '{override_tag}' instead of '{image_tag}'");

        image_tag = override_tag;
    }

    format!("{DOCKER_IMAGE}:{image_tag}")
}

fn get_container_name(cmd: &Cmd) -> String {
    if cmd.slop.contains(&"--name".to_string()) {
        cmd.slop[cmd.slop.iter().position(|x| x == "--name").unwrap() + 1].clone()
    } else {
        CONTAINER_NAME.to_string()
    }
}

/// The port mapping in the bollard crate is formatted differently than the docker CLI. In the docker CLI, we usually specify exposed ports as `-p  HOST_PORT:CONTAINER_PORT`. But with the bollard crate, it is expecting the port mapping to be a map of the container port (with the protocol) to the host port.
fn get_port_mapping(cmd: &Cmd) -> HashMap<String, Option<Vec<PortBinding>>> {
    let mut port_mapping = HashMap::new();
    if cmd.slop.contains(&"-p".to_string()) {
        let ports_string = cmd.slop[cmd.slop.iter().position(|x| x == "-p").unwrap() + 1].clone();
        let ports_vec: Vec<&str> = ports_string.split(':').collect();
        let from_port = ports_vec[0];
        let to_port = ports_vec[1];
        port_mapping.insert(
            format!("{to_port}/tcp"),
            Some(vec![PortBinding {
                host_ip: None,
                host_port: Some(from_port.to_string()),
            }]),
        );
    } else {
        port_mapping.insert(
            format!("{TO_PORT}/tcp"),
            Some(vec![PortBinding {
                host_ip: None,
                host_port: Some(format!("{FROM_PORT}")),
            }]),
        );
    }
    port_mapping
}

fn get_protocol_version_arg(cmd: &Cmd) -> String {
    if cmd.network == "local" && cmd.protocol_version.is_some() {
        let version = cmd.protocol_version.as_ref().unwrap();
        format!("--protocol-version {version}")
    } else {
        String::new()
    }
}

fn get_limits_arg(cmd: &Cmd) -> String {
    if cmd.network == "local" && cmd.limit.is_some() {
        let limit = cmd.limit.as_ref().unwrap();
        format!("--limits {limit}")
    } else {
        String::new()
    }
}
