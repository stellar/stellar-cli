use std::process::Command;

#[derive(thiserror::Error, Debug)]
pub enum Error {}

const CONTAINER_NAME: &str = "stellar";

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// docker container to stop
    pub container: Option<String>,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let docker_command;

        // let docker_container = &self.container.unwrap_or("stellar");

        match &self.container {
            Some(value) => {
                let container = value.to_string();
                docker_command = build_docker_command(container);
            }
            None => {
                let container = String::from(CONTAINER_NAME);
                docker_command = build_docker_command(container);
            }
        }
        run_docker_command(docker_command);
        Ok(())
    }
}

fn build_docker_command(container_name: String) -> String {
    let docker_command = format!(
        "docker stop {container_name}",
        container_name = container_name
    );

    docker_command
}

fn run_docker_command(docker_command: String) {
    // Use Command::new to create a new command
    let mut cmd = Command::new("sh");

    // Use arg method to add arguments to the command
    cmd.arg("-c").arg(docker_command);

    // Use output method to execute the command and capture the output
    let output = cmd.output().expect("Failed to execute command");

    // Check if the command was successful
    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout);
        println!("Docker command output: {}", result);
    } else {
        let result = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error executing Docker command: {}", result);
    }
}
