use serde::Deserialize;
use std::path::PathBuf;
use testcontainers::clients;

use speculos::{Args, Speculos};

pub mod emulator_http_transport;
pub mod speculos;

#[derive(Debug)]
pub struct LedgerTesting {
    host: String,
    local_elfs_dir: PathBuf,
    device_model: String,
    client: clients::Cli,
    pub transport_port: Option<u16>,
    speculos_api_port: Option<u16>,
}

const DEFAULT_HOST: &str = "localhost";
impl LedgerTesting {
    pub fn new(local_elfs_dir: PathBuf, device_model: String) -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            local_elfs_dir,
            device_model,
            client: clients::Cli::default(),

            transport_port: None,
            speculos_api_port: None,
        }
    }

    pub async fn start(&mut self) {
        let docker = &self.client;
        let container_args = Args {
            ledger_device_model: self.device_model.clone(),
        };

        let emulator_image = Speculos::new(self.local_elfs_dir.clone());

        let node = docker.run((emulator_image, container_args));

        let transport_port = node.get_host_port_ipv4(9998);
        let speculos_api_port = node.get_host_port_ipv4(5000);

        self.transport_port = Some(transport_port);
        self.speculos_api_port = Some(speculos_api_port);

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
        let mut ledger_testing = LedgerTesting::new(test_elfs_dir, "nanos".to_string());
        println!("Starting emulator: {:?}", ledger_testing);
        ledger_testing.start().await;
    }
}
