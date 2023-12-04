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
        let container = self
            .container
            .clone()
            .unwrap_or(String::from(CONTAINER_NAME));
        let docker_command = build_docker_command(&container);
        run_docker_command(&docker_command);
        Ok(())
    }
}

fn build_docker_command(container_name: &str) -> String {
    format!("docker stop {container_name}")
}

fn run_docker_command(docker_command: &str) {
    println!("Running docker command: `{docker_command}`");
    let output = Command::new("sh")
        .args(["-c", &docker_command])
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout);
        println!("Docker image stopped: {result}");
    } else {
        let result = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error executing Docker command: {result}");
    }
}
