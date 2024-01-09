use std::process::{Command, Stdio};

#[derive(thiserror::Error, Debug)]
pub enum Error {}

const FROM_PORT: i32 = 8000;
const TO_PORT: i32 = 8000;
const CONTAINER_NAME: &str = "stellar";
const DOCKER_IMAGE: &str = "stellar/quickstart";
const DOCKER_TAG: &str = "testing";

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// Network to start, e.g. local, testnet, futurenet
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

    /// optional argument to run docker process in detached mode
    #[arg(short = 'd', long)]
    pub detached: bool,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("Starting {} network", &self.network);

        let docker_command = build_docker_command(self);

        run_docker_command(&docker_command, self.detached);
        Ok(())
    }
}

fn build_docker_command(cmd: &Cmd) -> String {
    let image_tag = match cmd.network.as_str() {
        "local" => "latest",
        "pubnet" => "latest",
        "testnet" => "testing",
        "futurenet" => "soroban-dev",
        _ => "latest",
    };
    let image_name = format!("{}:{}", cmd.docker_image, image_tag);

    let docker_command = format!("docker run --rm {detached} -p \"{from_port}:{to_port}\" --name {container_name} {image_name} --{network} --enable-soroban-rpc",
      detached = if cmd.detached { "-d" } else { "" },
      from_port=FROM_PORT,
      to_port=TO_PORT,
      container_name=cmd.container_name,
      image_name=image_name,
      network=cmd.network
    );

    docker_command
}

fn run_docker_command(docker_command: &str, detached: bool) {
    println!("Running docker command: `{docker_command}`");
    if detached {
        println!("Running docker container id: ");
    }
    let mut cmd = Command::new("sh")
        .args(["-c", &docker_command])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    let status = cmd.wait();
    if status.is_err() {
        println!("Exited with status {status:?}");
    }
}
