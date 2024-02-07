use bollard::{ClientVersion, Docker};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    // #[error("Failed to execute docker command: {0}")]
    // CommandError(std::io::Error),

    // #[error("Failed to find docker container: {error}")]
    // ContainerNotFoundErr { error: String },
    #[error("Failed to stop container: {0}")]
    StopContainerError(#[from] bollard::errors::Error),
}

const CONTAINER_NAME: &str = "stellar";
// DEFAULT_TIMEOUT and API_DEFAULT_VERSION are from the bollard crate
const DEFAULT_TIMEOUT: u64 = 120;
const API_DEFAULT_VERSION: &ClientVersion = &ClientVersion {
    major_version: 1,
    minor_version: 40,
};

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// container to stop, defaults to "stellar"
    pub container_name: Option<String>,

    /// optional argument to override the default docker socket path
    #[arg(short = 'd', long)]
    pub docker_socket_path: Option<String>,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let container_name = self
            .container_name
            .clone()
            .unwrap_or(String::from(CONTAINER_NAME));

        println!("Stopping container: {container_name}");
        run_docker_command(self).await
    }
}

async fn run_docker_command(cmd: &Cmd) -> Result<(), Error> {
    let docker = connect_to_docker(cmd);
    let container_name = cmd
        .container_name
        .clone()
        .unwrap_or(String::from(CONTAINER_NAME));

    docker.stop_container(&container_name, None).await.unwrap();

    Ok(())
}

//TODO: move to a shared module
fn connect_to_docker(cmd: &Cmd) -> Docker {
    if cmd.docker_socket_path.is_some() {
        let socket = cmd.docker_socket_path.as_ref().unwrap();
        Docker::connect_with_socket(socket, DEFAULT_TIMEOUT, API_DEFAULT_VERSION).unwrap()
    } else {
        Docker::connect_with_socket_defaults().unwrap()
    }
}
