use std::env;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{
    commands::{
        container::shared::{Error as ConnectionError, Network},
        global,
    },
    print, rpc,
};

use super::shared::{Args, Name, RunArgs};

const DEFAULT_PORT_MAPPING: &str = "8000:8000";
const DOCKER_IMAGE: &str = "docker.io/stellar/quickstart";
const DEFAULT_WAIT_TIMEOUT_SECS: u64 = 120;
const HEALTH_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Docker(#[from] ConnectionError),

    #[error("failed to create container: {0}")]
    CreateContainerFailed(String),

    #[error("a container named {0:?} already running")]
    ContainerAlreadyRunning(String),

    #[error(transparent)]
    Rpc(#[from] rpc::Error),

    #[error("timed out after {timeout_secs}s waiting for network at {rpc_url} to become healthy")]
    HealthCheckTimeout { rpc_url: String, timeout_secs: u64 },
}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub container_args: Args,

    #[command(flatten)]
    pub run_args: RunArgs,

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

    /// Do not wait for the network RPC endpoint to report healthy before returning
    #[arg(long)]
    pub no_wait: bool,

    /// Maximum time in seconds to wait for the network RPC endpoint to become healthy
    #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
    pub wait_timeout: u64,
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

pub fn resolve_rpc_url(ports_mapping: &[String]) -> String {
    let mut fallback: Option<(String, String)> = None;

    for mapping in ports_mapping {
        let cleaned = mapping.trim().split('/').next().unwrap_or_default().trim();
        let parts: Vec<&str> = cleaned.split(':').collect();
        match parts.as_slice() {
            [host_port, container_port] if !host_port.is_empty() && !container_port.is_empty() => {
                if *container_port == "8000" {
                    return format!("http://localhost:{host_port}/rpc");
                }
                if fallback.is_none() {
                    fallback = Some(("localhost".to_string(), (*host_port).to_string()));
                }
            }
            [host_ip, host_port, container_port]
                if !host_ip.is_empty() && !host_port.is_empty() && !container_port.is_empty() =>
            {
                let host = if *host_ip == "0.0.0.0" {
                    "localhost"
                } else {
                    *host_ip
                };
                if *container_port == "8000" {
                    return format!("http://{host}:{host_port}/rpc");
                }
                if fallback.is_none() {
                    fallback = Some((host.to_string(), (*host_port).to_string()));
                }
            }
            _ => {}
        }
    }

    if let Some((host, port)) = fallback {
        format!("http://{host}:{port}/rpc")
    } else {
        "http://localhost:8000/rpc".to_string()
    }
}

pub async fn wait_for_healthy(
    rpc_url: &str,
    timeout: Duration,
    poll_interval: Duration,
    print: &print::Print,
) -> Result<(), Error> {
    let client = rpc::Client::new(rpc_url)?;
    let start = Instant::now();
    let mut last_status: Option<String> = None;

    print.infoln(format!(
        "Waiting for network at {rpc_url} to become healthy..."
    ));

    loop {
        if let Ok(resp) = client.get_health().await {
            if resp.status.eq_ignore_ascii_case("healthy") {
                print.checkln("Network is healthy");
                return Ok(());
            }
            if last_status.as_deref() != Some(resp.status.as_str()) {
                print.infoln(format!("Network status: {}", resp.status));
                last_status = Some(resp.status);
            }
        }

        if start.elapsed() >= timeout {
            return Err(Error::HealthCheckTimeout {
                rpc_url: rpc_url.to_string(),
                timeout_secs: timeout.as_secs(),
            });
        }

        tokio::time::sleep(poll_interval).await;
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
            .infoln(format!("Starting {} network", self.network));

        self.args.container_args.warn_if_host_ignored(&self.print);

        let image = self.get_image_name();
        self.pull_image(&image).await;

        let container_name = self.container_name().get_internal_container_name();
        let mut cmd = self
            .args
            .container_args
            .run_command(&container_name, &self.args.ports_mapping);
        self.args.run_args.apply(&mut cmd);
        cmd.arg(&image);
        // Each element of `get_container_args` is passed as a single argv token (some elements,
        // such as "--enable rpc,horizon,lab", intentionally contain spaces).
        for arg in self.get_container_args() {
            cmd.arg(arg);
        }

        let output = cmd
            .output()
            .await
            .map_err(|e| self.args.container_args.io_error(e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if self
                .args
                .container_args
                .is_container_already_running(&stderr)
            {
                return Err(Error::ContainerAlreadyRunning(container_name));
            }
            return Err(Error::CreateContainerFailed(stderr.trim().to_string()));
        }

        self.print.checkln("Started container");
        self.print_instructions();

        if !self.args.no_wait && self.is_rpc_enabled() {
            let rpc_url = resolve_rpc_url(&self.args.ports_mapping);
            wait_for_healthy(
                &rpc_url,
                Duration::from_secs(self.args.wait_timeout),
                HEALTH_POLL_INTERVAL,
                &self.print,
            )
            .await?;
        }

        Ok(())
    }

    fn is_rpc_enabled(&self) -> bool {
        let enabled =
            env::var("STELLAR_CONTAINER_ARGS").unwrap_or_else(|_| "rpc,horizon,lab".to_string());
        enabled
            .split(',')
            .any(|service| service.trim().eq_ignore_ascii_case("rpc"))
    }

    async fn pull_image(&self, image: &str) {
        let mut cmd = self.args.container_args.pull_command(image);
        cmd.stdout(Stdio::piped());

        // `docker pull` writes its progress to stderr; honor `--quiet` by discarding it.
        if self.print.quiet {
            cmd.stderr(Stdio::null());
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            // Let the main `docker run` invocation surface the "is docker installed?" hint
            // rather than warning about a failed image fetch.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => return self.warn_image_fetch(image),
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
        let mut image_tag = match &self.network {
            Network::Futurenet => "future",
            // pubnet, local, and testnet all default to latest
            _ => "latest",
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn test_resolve_rpc_url_default() {
        assert_eq!(
            resolve_rpc_url(&["8000:8000".to_string()]),
            "http://localhost:8000/rpc"
        );
    }

    #[test]
    fn test_resolve_rpc_url_custom_host_port() {
        assert_eq!(
            resolve_rpc_url(&["9090:8000/tcp".to_string()]),
            "http://localhost:9090/rpc"
        );
    }

    #[test]
    fn test_resolve_rpc_url_with_host_ip() {
        assert_eq!(
            resolve_rpc_url(&["127.0.0.1:18000:8000".to_string()]),
            "http://127.0.0.1:18000/rpc"
        );
        assert_eq!(
            resolve_rpc_url(&["0.0.0.0:18000:8000".to_string()]),
            "http://localhost:18000/rpc"
        );
    }

    #[test]
    fn test_cli_flags_no_wait_and_timeout() {
        let cmd =
            Cmd::try_parse_from(["start", "local", "--no-wait", "--wait-timeout", "45"]).unwrap();
        assert!(cmd.no_wait);
        assert_eq!(cmd.wait_timeout, 45);

        let default_cmd = Cmd::try_parse_from(["start"]).unwrap();
        assert!(!default_cmd.no_wait);
        assert_eq!(default_cmd.wait_timeout, DEFAULT_WAIT_TIMEOUT_SECS);
    }

    #[tokio::test]
    async fn test_wait_for_healthy_polls_until_healthy() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let attempt = Arc::new(AtomicUsize::new(0));
        let attempt_clone = Arc::clone(&attempt);

        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 2048];
                let _ = socket.read(&mut buf).await.unwrap();
                let idx = attempt_clone.fetch_add(1, Ordering::SeqCst);

                if idx == 0 {
                    // Simulate 502 Bad Gateway right after container starts
                    let resp = "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                    socket.write_all(resp.as_bytes()).await.unwrap();
                } else {
                    // Simulate healthy JSON-RPC response
                    let body = r#"{"jsonrpc":"2.0","id":1,"result":{"status":"healthy","latestLedger":10,"oldestLedger":1,"ledgerRetentionWindow":100}}"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    socket.write_all(resp.as_bytes()).await.unwrap();
                    break;
                }
            }
        });

        let rpc_url = format!("http://{addr}/rpc");
        let print = print::Print::new(true);
        wait_for_healthy(
            &rpc_url,
            Duration::from_secs(5),
            Duration::from_millis(50),
            &print,
        )
        .await
        .unwrap();

        assert_eq!(attempt.load(Ordering::SeqCst), 2);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn test_wait_for_healthy_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let _server = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let resp =
                    "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
            }
        });

        let rpc_url = format!("http://{addr}/rpc");
        let print = print::Print::new(true);
        let err = wait_for_healthy(
            &rpc_url,
            Duration::from_millis(120),
            Duration::from_millis(30),
            &print,
        )
        .await
        .unwrap_err();

        assert!(matches!(err, Error::HealthCheckTimeout { .. }));
    }
}
