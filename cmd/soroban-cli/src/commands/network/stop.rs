use std::process::Command;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to execute docker command: {0}")]
    CommandError(std::io::Error),

    #[error("Failed to find docker container: {error}")]
    ContainerNotFoundErr { error: String },
}

const CONTAINER_NAME: &str = "stellar";

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// docker container to stop, defaults to "stellar"
    pub container: Option<String>,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let container = self
            .container
            .clone()
            .unwrap_or(String::from(CONTAINER_NAME));
        let docker_command = build_docker_command(&container);
        run_docker_command(&docker_command)
    }
}

fn build_docker_command(container_name: &str) -> String {
    format!("docker stop {container_name}")
}

fn run_docker_command(docker_command: &str) -> Result<(), Error> {
    println!("Running docker command: `{docker_command}`");
    let output = Command::new("sh")
        .args(["-c", &docker_command])
        .output()
        .map_err(Error::CommandError)
        .and_then(|output| {
            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout);
                println!("Docker image stopped: {result}");
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                Err(Error::ContainerNotFoundErr { error: stderr })
            }
        });
    output
}
