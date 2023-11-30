use std::process::Command;

#[derive(thiserror::Error, Debug)]
pub enum Error {}

const FROM_PORT: i32 = 8000;
const TO_PORT: i32 = 8000;
const CONTAINER_NAME: &str = "stellar";
const DOCKER_IMAGE: &str = "stellar/quickstart";
const DOCKER_TAG: &str = "testing";

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// Network to start
    pub network: String,

    /// optional argument for a custom local container name, defaults to "stellar"
    #[arg(short, long, default_value=CONTAINER_NAME)]
    pub container_name: String,

    /// optional argument for a different docker image, defaults to "stellar/quickstart"
    #[arg(short = 'i', long, default_value=DOCKER_IMAGE)]
    pub docker_image: String,

    /// optional argument for a different docker tag, defaults to "testing"
    #[arg(short = 't', long, default_value=DOCKER_TAG)]
    pub docker_tag: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("Starting {}", &self.network);
        start_container(self);
        Ok(())
    }
}

fn build_docker_command(cmd: &Cmd) -> String {
    let image_name = format!("{}:{}", cmd.docker_image, cmd.docker_tag);
    let docker_command = format!("docker run --rm -d -p \"{from_port}:{to_port}\" --name {container_name} {image_name} --{network} --enable-soroban-rpc",
      from_port=FROM_PORT,
      to_port=TO_PORT,
      container_name=cmd.container_name,
      image_name=image_name,
      network=cmd.network
    );

    docker_command
}

fn start_container(cmd: &Cmd) {
    let docker_command = build_docker_command(&cmd);

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
