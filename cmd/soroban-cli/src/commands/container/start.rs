use std::env;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{
    commands::{
        container::shared::{Error as ConnectionError, Network},
        global,
    },
    print,
};

use super::shared::{Args, Name};

const DEFAULT_PORT_MAPPING: &str = "8000:8000";
const DOCKER_IMAGE: &str = "docker.io/stellar/quickstart";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Docker(#[from] ConnectionError),

    #[error("failed to create container: {0}")]
    CreateContainerFailed(String),

    #[error("a container named {0:?} already running")]
    ContainerAlreadyRunning(String),
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: Args,

    /// Network to start. Default is `local`
    pub network: Option<Network>,

    /// Optional argument to specify the container name
    #[arg(long)]
    pub name: Option<String>,

    /// Optional argument to specify the limits for the local network only
    #[arg(short = 'l', long)]
    pub limits: Option<String>,

    /// Argument to specify the `HOST_PORT:CONTAINER_PORT` mapping
    #[arg(short = 'p', long, num_args = 1.., default_value = DEFAULT_PORT_MAPPING)]
    pub ports_mapping: Vec<String>,

    /// Optional argument to override the default docker image tag for the given network
    #[arg(short = 't', long)]
    pub image_tag_override: Option<String>,

    /// Optional argument to specify the protocol version for the local network only
    #[arg(long)]
    pub protocol_version: Option<String>,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let runner = Runner {
            args: self.clone(),
            network: self.network.unwrap_or(Network::Local),
            print: print::Print::new(global_args.quiet),
        };

        runner.run_docker_command().await
    }
}

struct Runner {
    args: Cmd,
    network: Network,
    print: print::Print,
}

impl Runner {
    async fn run_docker_command(&self) -> Result<(), Error> {
        self.print
            .infoln(format!("Starting {} network", &self.network));

        let image = self.get_image_name();
        self.pull_image(&image).await;

        let container_name = self.container_name().get_internal_container_name();
        let mut cmd = self.args.container_args.docker_command();
        cmd.args(["run", "-d", "--rm", "--name", &container_name]);
        for port_mapping in &self.args.ports_mapping {
            cmd.args(["-p", port_mapping]);
        }
        cmd.arg(&image);
        // Each element of `get_container_args` is passed as a single argv token (some elements,
        // such as "--enable rpc,horizon,lab", intentionally contain spaces).
        for arg in self.get_container_args() {
            cmd.arg(arg);
        }

        let output = cmd.output().await.map_err(ConnectionError::from)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("already in use") {
                return Err(Error::ContainerAlreadyRunning(container_name));
            }
            return Err(Error::CreateContainerFailed(stderr.trim().to_string()));
        }

        self.print.checkln("Started container");
        self.print_instructions();
        Ok(())
    }

    async fn pull_image(&self, image: &str) {
        let mut cmd = self.args.container_args.docker_command();
        cmd.args(["pull", image]).stdout(Stdio::piped());

        let Ok(mut child) = cmd.spawn() else {
            return self.warn_image_fetch(image);
        };

        if let Some(stdout) = child.stdout.take() {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("Pulling from")
                    || line.contains("Digest")
                    || line.contains("Status")
                {
                    self.print.infoln(line);
                }
            }
        }

        match child.wait().await {
            Ok(status) if status.success() => {}
            _ => self.warn_image_fetch(image),
        }
    }

    fn warn_image_fetch(&self, image: &str) {
        self.print
            .warnln(format!("Failed to fetch image: {image}."));
        self.print
            .warnln("Attempting to start local quickstart image. The image may be out-of-date.");
    }

    fn get_image_name(&self) -> String {
        // this can be overriden with the `-t` flag
        let mut image_tag = match &self.network {
            Network::Pubnet => "latest",
            Network::Futurenet => "future",
            _ => "testing", // default to testing for local and testnet
        };

        if let Some(image_override) = &self.args.image_tag_override {
            self.print.infoln(format!(
                "Overriding docker image tag to use '{image_override}' instead of '{image_tag}'"
            ));
            image_tag = image_override;
        }

        format!("{DOCKER_IMAGE}:{image_tag}")
    }

    fn get_container_args(&self) -> Vec<String> {
        let args = env::var("STELLAR_CONTAINER_ARGS").unwrap_or("rpc,horizon,lab".to_string());

        [
            format!("--{}", self.network),
            format!("--enable {args}"),
            self.get_protocol_version_arg(),
            self.get_limits_arg(),
        ]
        .iter()
        .filter(|&s| !s.is_empty())
        .cloned()
        .collect()
    }

    fn container_name(&self) -> Name {
        Name(self.args.name.clone().unwrap_or(self.network.to_string()))
    }

    fn print_instructions(&self) {
        let container_name = self.container_name().get_external_container_name().clone();
        let additional_flags = self.args.container_args.get_additional_flags().clone();
        let tail = format!("{container_name} {additional_flags}");

        self.print.searchln(format!(
            "Watch logs with `stellar container logs {}`",
            tail.trim()
        ));

        self.print.infoln(format!(
            "Stop the container with `stellar container stop {}`",
            tail.trim()
        ));
    }

    fn get_protocol_version_arg(&self) -> String {
        if self.network == Network::Local {
            if let Some(version) = self.args.protocol_version.as_ref() {
                return format!("--protocol-version {version}");
            }
        }

        String::new()
    }

    fn get_limits_arg(&self) -> String {
        if self.network == Network::Local {
            if let Some(limits) = self.args.limits.as_ref() {
                return format!("--limits {limits}");
            }
        }

        String::new()
    }
}
