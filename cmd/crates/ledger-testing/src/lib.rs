use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, thread::sleep, time::Duration};
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

        self.wait_for_emulator_start_text().await;
    }

    pub fn get_transport_port(&self) -> u16 {
        self.transport_port.unwrap()
    }

    pub fn get_speculos_api_port(&self) -> u16 {
        self.speculos_api_port.unwrap()
    }

    async fn wait_for_emulator_start_text(&self) {
        let mut ready = false;
        while !ready {
            if self
                .get_emulator_events()
                .await
                .iter()
                .any(|event| event.text == "is ready")
            {
                ready = true;
            }
        }
    }

    pub async fn get_emulator_events(&self) -> Vec<EmulatorEvent> {
        let host = &self.host;
        let port = self.speculos_api_port.unwrap();
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://{host}:{port}/events"))
            .send()
            .await
            .unwrap()
            .json::<EventsResponse>()
            .await
            .unwrap(); // not worrying about unwraps for test helpers for now
        resp.events
    }

    // TODO: make button into an enum
    pub async fn click(&self, button: &str) {
        let host = &self.host;
        let port = self.speculos_api_port.unwrap();

        let previous_events = self.get_emulator_events().await;

        let http_client = reqwest::Client::new();
        let mut payload = HashMap::new();
        payload.insert("action", "press-and-release");

        let mut screen_has_changed = false;

        http_client
            .post(format!("http://{host}:{port}/button/{button}"))
            .json(&payload)
            .send()
            .await
            .unwrap();

        while !screen_has_changed {
            let current_events = self.get_emulator_events().await;

            if !(previous_events == current_events) {
                screen_has_changed = true
            }
        }

        sleep(Duration::from_secs(1));
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct EmulatorEvent {
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

        // it exposes the transport port
        assert!(ledger_testing.get_transport_port() > 0);

        // it exposes the speculos api port
        assert!(ledger_testing.get_speculos_api_port() > 0);

        // it gets the emulator events and waits for the emulator to be ready
        let events = ledger_testing.get_emulator_events().await;
        assert!(events.len() > 0);
        assert!(events.iter().any(|event| event.text == "is ready"));
    }

    #[tokio::test]
    async fn test_clicking_the_left_button() {
        let test_elfs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/test_elfs");
        let docker = Cli::default();
        let mut ledger_testing = LedgerTesting::new(test_elfs_dir, "nanos".to_string());
        ledger_testing.start(&docker).await;

        ledger_testing.click("left").await;
        let events = ledger_testing.get_emulator_events().await;

        // on a nano s, after the "Stellar is Ready" screen appears, when you click the "left" button you get a screen that says "Quit"
        assert!(events.iter().any(|event| event.text == "Quit"));
    }

    #[tokio::test]
    async fn test_clicking_the_right_button() {
        let test_elfs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/test_elfs");
        let docker = Cli::default();
        let mut ledger_testing = LedgerTesting::new(test_elfs_dir, "nanos".to_string());
        ledger_testing.start(&docker).await;

        ledger_testing.click("right").await;
        let events = ledger_testing.get_emulator_events().await;

        // on a nano s, after the "Stellar is Ready" screen appears, when you click the "right" button you get a screen that says "Settings"
        assert!(events.iter().any(|event| event.text == "Settings"));
    }

    #[tokio::test]
    async fn test_clicking_the_both_button() {
        let test_elfs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/test_elfs");
        let docker = Cli::default();
        let mut ledger_testing = LedgerTesting::new(test_elfs_dir, "nanos".to_string());
        ledger_testing.start(&docker).await;

        ledger_testing.click("right").await;
        ledger_testing.click("both").await;
        let events = ledger_testing.get_emulator_events().await;

        // on a nano s, after the "Stellar is Ready" screen appears, when you click the "right" button and then the "both" button you get a screen that says "Hash signing" "NOT Enabled" (as two separate events)
        assert!(events.iter().any(|event| event.text == "Hash signing"));
        assert!(events.iter().any(|event| event.text == "NOT Enabled"));
    }

}
