use serde::Deserialize;
use std::path::PathBuf;
use testcontainers::{clients::Cli, Container};

use speculos::{Args, Speculos};

pub mod emulator_http_transport;
pub mod speculos;

const DEFAULT_HOST: &str = "localhost";
const TRANSPORT_PORT: u16 = 9998;
const SPECULOS_API_PORT: u16 = 5000;

#[derive(Debug)]
pub struct LedgerTesting<'a> {
    host: String,
    local_elfs_dir: PathBuf,
    device_model: String,
    transport_port: Option<u16>,
    speculos_api_port: Option<u16>,
    container: Option<Container<'a, Speculos>>,
}

impl<'a> LedgerTesting<'a> {
    pub fn new(local_elfs_dir: PathBuf, device_model: String) -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            local_elfs_dir,
            device_model,
            transport_port: None,
            speculos_api_port: None,
            container: None,
        }
    }

    pub async fn start(&mut self, docker: &'a Cli) {
        let container_args = Args {
            ledger_device_model: self.device_model.clone(),
        };

        let emulator_image = Speculos::new(self.local_elfs_dir.clone());

        let container = docker.run((emulator_image, container_args));

        let transport_port = container.get_host_port_ipv4(TRANSPORT_PORT);
        let speculos_api_port = container.get_host_port_ipv4(SPECULOS_API_PORT);

        self.transport_port = Some(transport_port);
        self.speculos_api_port = Some(speculos_api_port);
        self.container = Some(container);

        wait_for_emulator_start_text(speculos_api_port).await;
    }
}

async fn wait_for_emulator_start_text(ui_host_port: u16) {
    let mut ready = false;
    while !ready {
        if get_emulator_events(ui_host_port)
            .await
            .iter()
            .any(|event| event.text == "is ready")
        {
            ready = true;
        }
    }
}

async fn get_emulator_events(ui_host_port: u16) -> Vec<EmulatorEvent> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://localhost:{ui_host_port}/events"))
        .send()
        .await
        .unwrap()
        .json::<EventsResponse>()
        .await
        .unwrap(); // not worrying about unwraps for test helpers for now
    resp.events
}

#[derive(Debug, Deserialize, PartialEq)]
struct EmulatorEvent {
    text: String,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
}

#[derive(Debug, Deserialize)]
struct EventsResponse {
    events: Vec<EmulatorEvent>,
}

#[cfg(test)]
mod test {
    use super::*;
    #[tokio::test]
    async fn test_start_nano_s() {
        let test_elfs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/test_elfs");
        let docker = Cli::default();
        let mut ledger_testing = LedgerTesting::new(test_elfs_dir, "nanos".to_string());
        ledger_testing.start(&docker).await;

        assert!(ledger_testing.transport_port.is_some());
    }
}
