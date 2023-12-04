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
    println!("Running docker command: `{}`", docker_command);
    let output = Command::new("sh")
        .args(&["-c", &docker_command])
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout);
        println!("Docker image stopped: {}", result);
    } else {
        let result = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error executing Docker command: {}", result);
    }
}
