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

    /// optional argument to customize container name
    #[arg(short, long, default_value=CONTAINER_NAME)]
    pub container_name: String,

    /// optional argument for docker image
    #[arg(short = 'i', long, default_value=DOCKER_IMAGE)]
    pub docker_image: String,

    /// optional argument for docker tag
    #[arg(short = 't', long, default_value=DOCKER_TAG)]
    pub docker_tag: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("Starting {} network", &self.network);
        let docker_command = build_docker_command(&self);

        run_docker_command(docker_command);
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

fn run_docker_command(docker_command: String) {
    println!("Running docker command: `{}`", docker_command);
    let output = Command::new("sh")
        .args(&["-c", &docker_command])
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout);
        println!("Docker container id started: {}", result);
    } else {
        let result = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error executing Docker command: {}", result);
    }
}
