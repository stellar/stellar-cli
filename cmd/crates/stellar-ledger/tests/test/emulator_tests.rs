use crate::xdr::{self, Operation, OperationBody, Uint256};
use crate::xdr::{Hash, Transaction};
use ledger_transport::Exchange;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::ops::Range;
use std::sync::Mutex;
use std::vec;

use std::net::TcpListener;
use stellar_ledger::hd_path::HdPath;
use stellar_ledger::{Blob, Error, LedgerSigner};

use std::sync::Arc;
use std::{collections::HashMap, time::Duration};

use stellar_xdr::curr::{
    Memo, MuxedAccount, PaymentOp, Preconditions, SequenceNumber, TransactionExt,
};
use testcontainers::{core::ContainerPort, runners::AsyncRunner, ContainerAsync, ImageExt};
use tokio::time::sleep;

static PORT_RANGE: Lazy<Mutex<Range<u16>>> = Lazy::new(|| Mutex::new(40000..50000));

pub const TEST_NETWORK_PASSPHRASE: &[u8] = b"Test SDF Network ; September 2015";
pub fn test_network_hash() -> Hash {
    use sha2::Digest;
    Hash(sha2::Sha256::digest(TEST_NETWORK_PASSPHRASE).into())
}

async fn ledger(host_port: u16) -> LedgerSigner<impl Exchange> {
    LedgerSigner::new(get_http_transport("127.0.0.1", host_port).await.unwrap())
}

mod test_helpers {
    pub mod test {
        include!("../utils/mod.rs");
    }
}

use test_case::test_case;
use test_helpers::test::{emulator_http_transport::EmulatorHttpTransport, speculos::Speculos};

#[test_case("nanos".to_string() ; "when the device is NanoS")]
#[test_case("nanox".to_string() ; "when the device is NanoX")]
#[test_case("nanosp".to_string() ; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_get_public_key(ledger_device_model: String) {
    let container = get_container(ledger_device_model.clone()).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();
    wait_for_emulator_start_text(ui_host_port).await;

    let ledger = ledger(host_port).await;

    match ledger.get_public_key(&0.into()).await {
        Ok(public_key) => {
            let public_key_string = public_key.to_string();
            // This is determined by the seed phrase used to start up the emulator
            // TODO: make the seed phrase configurable
            let expected_public_key = "GDUTHCF37UX32EMANXIL2WOOVEDZ47GHBTT3DYKU6EKM37SOIZXM2FN7";
            assert_eq!(public_key_string, expected_public_key);
        }
        Err(e) => {
            println!("{e}");
            assert!(false);
        }
    }
}

#[test_case("nanos".to_string() ; "when the device is NanoS")]
#[test_case("nanox".to_string() ; "when the device is NanoX")]
#[test_case("nanosp".to_string() ; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_get_app_configuration(ledger_device_model: String) {
    let container = get_container(ledger_device_model.clone()).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();
    wait_for_emulator_start_text(ui_host_port).await;

    let ledger = ledger(host_port).await;

    match ledger.get_app_configuration().await {
        Ok(config) => {
            assert_eq!(config, vec![0, 5, 0, 3]);
        }
        Err(e) => {
            println!("{e}");
            assert!(false);
        }
    };
}

#[test_case("nanos".to_string() ; "when the device is NanoS")]
#[test_case("nanox".to_string() ; "when the device is NanoX")]
#[test_case("nanosp".to_string() ; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_sign_tx(ledger_device_model: String) {
    let container = get_container(ledger_device_model.clone()).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();
    wait_for_emulator_start_text(ui_host_port).await;

    let ledger = Arc::new(ledger(host_port).await);

    let path = HdPath(0);

    let source_account_str = "GAQNVGMLOXSCWH37QXIHLQJH6WZENXYSVWLPAEF4673W64VRNZLRHMFM";
    let source_account_bytes = match stellar_strkey::Strkey::from_string(source_account_str) {
        Ok(key) => match key {
            stellar_strkey::Strkey::PublicKeyEd25519(p) => p.0,
            _ => {
                eprintln!("Error decoding public key: {:?}", key);
                return;
            }
        },
        Err(err) => {
            eprintln!("Error decoding public key: {}", err);
            return;
        }
    };

    let destination_account_str = "GCKUD4BHIYSAYHU7HBB5FDSW6CSYH3GSOUBPWD2KE7KNBERP4BSKEJDV";
    let destination_account_bytes =
        match stellar_strkey::Strkey::from_string(destination_account_str) {
            Ok(key) => match key {
                stellar_strkey::Strkey::PublicKeyEd25519(p) => p.0,
                _ => {
                    eprintln!("Error decoding public key: {:?}", key);
                    return;
                }
            },
            Err(err) => {
                eprintln!("Error decoding public key: {}", err);
                return;
            }
        };

    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(source_account_bytes)),
        fee: 100,
        seq_num: SequenceNumber(1),
        cond: Preconditions::None,
        memo: Memo::Text("Stellar".as_bytes().try_into().unwrap()),
        ext: TransactionExt::V0,
        operations: [Operation {
            source_account: Some(MuxedAccount::Ed25519(Uint256(source_account_bytes))),
            body: OperationBody::Payment(PaymentOp {
                destination: MuxedAccount::Ed25519(Uint256(destination_account_bytes)),
                asset: xdr::Asset::Native,
                amount: 100,
            }),
        }]
        .try_into()
        .unwrap(),
    };

    let sign = tokio::task::spawn({
        let ledger = Arc::clone(&ledger);
        async move { ledger.sign_transaction(path, tx, test_network_hash()).await }
    });
    let approve = tokio::task::spawn(approve_tx_signature(ui_host_port, ledger_device_model));

    let result = sign.await.unwrap();
    let _ = approve.await.unwrap();

    match result {
        Ok(response) => {
            assert_eq!( hex::encode(response), "5c2f8eb41e11ab922800071990a25cf9713cc6e7c43e50e0780ddc4c0c6da50c784609ef14c528a12f520d8ea9343b49083f59c51e3f28af8c62b3edeaade60e");
        }
        Err(e) => {
            println!("{e}");
            assert!(false);
        }
    };
}

#[test_case("nanos".to_string() ; "when the device is NanoS")]
#[test_case("nanox".to_string() ; "when the device is NanoX")]
#[test_case("nanosp".to_string() ; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_sign_tx_hash_when_hash_signing_is_not_enabled(ledger_device_model: String) {
    let container = get_container(ledger_device_model.clone()).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();
    wait_for_emulator_start_text(ui_host_port).await;

    let ledger = ledger(host_port).await;

    let path = 0;
    let test_hash = b"313e8447f569233bb8db39aa607c8889";

    let result = ledger.sign_transaction_hash(path, test_hash).await;
    if let Err(Error::APDUExchangeError(msg)) = result {
        assert_eq!(msg, "Ledger APDU retcode: 0x6C66");
        // this error code is SW_TX_HASH_SIGNING_MODE_NOT_ENABLED https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
    } else {
        panic!("Unexpected result: {:?}", result);
    }
}

#[test_case("nanos".to_string() ; "when the device is NanoS")]
#[test_case("nanox".to_string() ; "when the device is NanoX")]
#[test_case("nanosp".to_string() ; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_sign_tx_hash_when_hash_signing_is_enabled(ledger_device_model: String) {
    let container = get_container(ledger_device_model.clone()).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();

    wait_for_emulator_start_text(ui_host_port).await;
    enable_hash_signing(ui_host_port).await;

    let ledger = Arc::new(ledger(host_port).await);

    let path = 0;
    let mut test_hash = [0u8; 32];

    match hex::decode_to_slice(
        "313e8447f569233bb8db39aa607c8889313e8447f569233bb8db39aa607c8889",
        &mut test_hash as &mut [u8],
    ) {
        Ok(()) => {}
        Err(e) => {
            panic!("Unexpected result: {e}");
        }
    }

    let sign = tokio::task::spawn({
        let ledger = Arc::clone(&ledger);
        async move { ledger.sign_transaction_hash(path, &test_hash).await }
    });
    let approve = tokio::task::spawn(approve_tx_hash_signature(ui_host_port, ledger_device_model));

    let response = sign.await.unwrap();
    let _ = approve.await.unwrap();

    match response {
        Ok(response) => {
            assert_eq!( hex::encode(response), "e0fa9d19f34ddd494bbb794645fc82eb5ebab29e74160f1b1d5697e749aada7c6b367236df87326b0fdc921ed39702242fc8b14414f4e0ee3e775f1fd0208101");
        }
        Err(e) => {
            panic!("Unexpected result: {e}");
        }
    }
}

async fn click(ui_host_port: u16, url: &str) {
    let previous_events = get_emulator_events(ui_host_port).await;

    let client = reqwest::Client::new();
    let mut payload = HashMap::new();
    payload.insert("action", "press-and-release");

    let mut screen_has_changed = false;

    client
        .post(format!("http://localhost:{ui_host_port}/{url}"))
        .json(&payload)
        .send()
        .await
        .unwrap();

    while !screen_has_changed {
        let current_events = get_emulator_events(ui_host_port).await;

        if !(previous_events == current_events) {
            screen_has_changed = true
        }
    }

    sleep(Duration::from_secs(1)).await;
}

async fn enable_hash_signing(ui_host_port: u16) {
    click(ui_host_port, "button/right").await;

    click(ui_host_port, "button/both").await;

    click(ui_host_port, "button/both").await;

    click(ui_host_port, "button/right").await;

    click(ui_host_port, "button/right").await;

    click(ui_host_port, "button/both").await;
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

async fn get_container(ledger_device_model: String) -> ContainerAsync<Speculos> {
    let (tcp_port_1, tcp_port_2) = get_available_ports(2);
    Speculos::new(ledger_device_model)
        .with_mapped_port(tcp_port_1, ContainerPort::Tcp(9998))
        .with_mapped_port(tcp_port_2, ContainerPort::Tcp(5000))
        .start()
        .await
        .unwrap()
}

fn get_available_ports(n: usize) -> (u16, u16) {
    let mut range = PORT_RANGE.lock().unwrap();
    let mut ports = Vec::with_capacity(n);
    while ports.len() < n {
        if let Some(port) = range.next() {
            if let Ok(listener) = TcpListener::bind(("0.0.0.0", port)) {
                ports.push(port);
                drop(listener);
            }
        } else {
            panic!("No more available ports");
        }
    }

    (ports[0], ports[1])
}

async fn get_http_transport(host: &str, port: u16) -> Result<impl Exchange, Error> {
    let max_retries = 5;
    let mut retries = 0;
    let mut wait_time = Duration::from_secs(1);
    // ping the emulator port to make sure it's up and running
    // retry with exponential backoff
    loop {
        match reqwest::get(format!("http://{host}:{port}")).await {
            Ok(_) => return Ok(EmulatorHttpTransport::new(host, port)),
            Err(e) => {
                retries += 1;
                if retries >= max_retries {
                    println!("get_http_transport: Exceeded max retries for connecting to emulated device");

                    return Err(Error::APDUExchangeError(format!(
                        "Failed to connect to emulator: {e}"
                    )));
                }
                sleep(wait_time).await;
                wait_time *= 2;
            }
        }
    }
}

async fn wait_for_emulator_start_text(ui_host_port: u16) {
    let mut ready = false;
    while !ready {
        let events = get_emulator_events_with_retries(ui_host_port, 5).await;

        if events.iter().any(|event| event.text == "is ready") {
            ready = true;
        }
    }
}

async fn get_emulator_events(ui_host_port: u16) -> Vec<EmulatorEvent> {
    // Allowing for less retries here because presumably the emulator should be up and running since we waited
    // for the "is ready" text via wait_for_emulator_start_text
    get_emulator_events_with_retries(ui_host_port, 1).await
}

async fn get_emulator_events_with_retries(
    ui_host_port: u16,
    max_retries: u16,
) -> Vec<EmulatorEvent> {
    let client = reqwest::Client::new();
    let mut retries = 0;
    let mut wait_time = Duration::from_secs(1);
    loop {
        match client
            .get(format!("http://localhost:{ui_host_port}/events"))
            .send()
            .await
        {
            Ok(req) => {
                let resp = req.json::<EventsResponse>().await.unwrap();
                return resp.events;
            }
            Err(e) => {
                retries += 1;
                if retries >= max_retries {
                    println!("get_emulator_events_with_retries: Exceeded max retries");
                    panic!("get_emulator_events_with_retries: Failed to get emulator events: {e}");
                }
                sleep(wait_time).await;
                wait_time *= 2;
            }
        }
    }
}

async fn approve_tx_hash_signature(ui_host_port: u16, device_model: String) {
    let number_of_right_clicks = if device_model == "nanos" { 10 } else { 6 };
    for _ in 0..number_of_right_clicks {
        click(ui_host_port, "button/right").await;
    }

    click(ui_host_port, "button/both").await;
}

async fn approve_tx_signature(ui_host_port: u16, device_model: String) {
    let number_of_right_clicks = if device_model == "nanos" { 17 } else { 11 };
    for _ in 0..number_of_right_clicks {
        click(ui_host_port, "button/right").await;
    }
    click(ui_host_port, "button/both").await;
}
