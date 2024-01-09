use std::process::{Command, Stdio};

#[derive(thiserror::Error, Debug)]
pub enum Error {}

const FROM_PORT: i32 = 8000;
const TO_PORT: i32 = 8000;
const CONTAINER_NAME: &str = "stellar";
const DOCKER_IMAGE: &str = "stellar/quickstart";

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

    // optional arguments to pass to the docker run command
    #[arg(last = true, id = "DOCKER_RUN_ARGS")]
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

fn build_docker_command(cmd: &Cmd) -> String {
    let image = get_image_name(cmd);
    let container_name = get_container_name(cmd);
    let port_mapping = get_port_mapping(cmd);

    let docker_command =
        format!(
        "docker run --rm {slop} {port} {container_name} {image} --{network} {enable_soroban_rpc}",
        port = format_args!("-p {port_mapping}"),
        container_name = format_args!("--name {container_name}"),
        image = image,
        network = cmd.network,
        slop = cmd.slop.join(" "),
        enable_soroban_rpc = if cmd.disable_soroban_rpc { "" } else { "--enable-soroban-rpc" },
    );

    docker_command
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

fn get_port_mapping(cmd: &Cmd) -> String {
    if cmd.slop.contains(&"-p".to_string()) {
        cmd.slop[cmd.slop.iter().position(|x| x == "-p").unwrap() + 1].clone()
    } else {
        format!("{FROM_PORT}:{TO_PORT}")
    }
}
