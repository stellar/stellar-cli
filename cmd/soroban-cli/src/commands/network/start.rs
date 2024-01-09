use std::process::{Command, Stdio};

#[derive(thiserror::Error, Debug)]
pub enum Error {}

const FROM_PORT: i32 = 8000;
const TO_PORT: i32 = 8000;
const CONTAINER_NAME: &str = "stellar";
const DOCKER_IMAGE: &str = "stellar/quickstart";

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    /// Network to start, e.g. local, testnet, futurenet
    pub network: String,

    /// optional argument to override the default docker image tag for the given network
    #[arg(short = 't', long)]
    pub image_tag_override: Option<String>,

    // optional arguments to pass to docker command
    #[arg(last = true, id = "DOCKER_COMMAND_ARGS")]
    pub slop: Vec<String>,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("Starting {} network", &self.network);

        let docker_command = build_docker_command(self);

        run_docker_command(&docker_command);
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

    if cmd.image_tag_override.is_some() {
        let override_tag = cmd.image_tag_override.as_ref().unwrap();
        println!(
            "Overriding docker image tag to use '{}' instead of '{}'",
            override_tag, image_tag
        );

        image_tag = override_tag;
    }

    let image = format!("{}:{}", DOCKER_IMAGE, image_tag);

    let container_name = if cmd.slop.contains(&"--name".to_string()) {
        cmd.slop[cmd.slop.iter().position(|x| x == "--name").unwrap() + 1].clone()
    } else {
        CONTAINER_NAME.to_string()
    };

    let port = if cmd.slop.contains(&"-p".to_string()) {
        cmd.slop[cmd.slop.iter().position(|x| x == "-p").unwrap() + 1].clone()
    } else {
        format!("{}:{}", FROM_PORT, TO_PORT)
    };

    let docker_command = format!(
        "docker run --rm {slop} {port} {container_name} {image} --{network} --enable-soroban-rpc",
        port = format!("-p {port}"),
        container_name = format!("--name {container_name}"),
        image = image,
        network = cmd.network,
        slop = cmd.slop.join(" ")
    );

    docker_command
}

fn run_docker_command(docker_command: &str) {
    println!("Running docker command: `{docker_command}`");
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
