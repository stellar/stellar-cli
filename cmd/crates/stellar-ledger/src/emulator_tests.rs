use soroban_env_host::xdr::Transaction;
use std::vec;

use crate::signer::Stellar;
use serde::Deserialize;
use soroban_env_host::xdr::{self, Operation, OperationBody, Uint256};

use crate::speculos::Speculos;
use crate::{get_zemu_transport, LedgerError, LedgerOptions, LedgerSigner};

use std::sync::Arc;
use std::{collections::HashMap, str::FromStr, time::Duration};

use stellar_xdr::curr::{
    Memo, MuxedAccount, PaymentOp, Preconditions, SequenceNumber, TransactionExt,
};

use testcontainers::clients;
use tokio::time::sleep;

const TEST_NETWORK_PASSPHRASE: &str = "Test SDF Network ; September 2015";

// #[ignore]
// #[tokio::test]
// #[serial]
// async fn test_get_public_key_with_ledger_device() {
//     let transport = get_transport().unwrap();
//     let ledger_options = Some(LedgerOptions {
//         exchange: transport,
//         hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
//     });
//     let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options);
//     let public_key = ledger.get_public_key(0).await;
//     assert!(public_key.is_ok());
// }

#[tokio::test]
async fn test_get_public_key() {
    let docker = clients::Cli::default();
    let node = docker.run(Speculos::new());
    let host_port = node.get_host_port_ipv4(9998);
    let ui_host_port: u16 = node.get_host_port_ipv4(5000);

    wait_for_emulator_start_text(ui_host_port).await;

    let transport = get_zemu_transport("127.0.0.1", host_port).unwrap();
    let ledger_options = Some(LedgerOptions {
        exchange: transport,
        hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
    });
    let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options);

    match ledger.get_public_key(0).await {
        Ok(public_key) => {
            let public_key_string = public_key.to_string();
            // This is determined by the seed phrase used to start up the emulator
            // TODO: make the seed phrase configurable
            let expected_public_key = "GDUTHCF37UX32EMANXIL2WOOVEDZ47GHBTT3DYKU6EKM37SOIZXM2FN7";
            assert_eq!(public_key_string, expected_public_key);
        }
        Err(e) => {
            node.stop();
            println!("{e}");
            assert!(false);
        }
    }

    node.stop();
}

#[tokio::test]
async fn test_get_app_configuration() {
    let docker = clients::Cli::default();
    let node = docker.run(Speculos::new());
    let host_port = node.get_host_port_ipv4(9998);
    let ui_host_port: u16 = node.get_host_port_ipv4(5000);

    wait_for_emulator_start_text(ui_host_port).await;

    let transport = get_zemu_transport("127.0.0.1", host_port).unwrap();
    let ledger_options = Some(LedgerOptions {
        exchange: transport,
        hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
    });
    let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options);

    match ledger.get_app_configuration().await {
        Ok(config) => {
            assert_eq!(config, vec![0, 5, 0, 3]);
        }
        Err(e) => {
            node.stop();
            println!("{e}");
            assert!(false);
        }
    };

    node.stop();
}

#[tokio::test]
async fn test_sign_tx() {
    let docker = clients::Cli::default();
    let node = docker.run(Speculos::new());
    let host_port = node.get_host_port_ipv4(9998);
    let ui_host_port: u16 = node.get_host_port_ipv4(5000);

    wait_for_emulator_start_text(ui_host_port).await;

    let transport = get_zemu_transport("127.0.0.1", host_port).unwrap();
    let ledger_options = Some(LedgerOptions {
        exchange: transport,
        hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
    });
    let ledger = Arc::new(LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options));

    let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();

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
        source_account: MuxedAccount::Ed25519(Uint256(source_account_bytes)), // was struggling to create a real account in this way with the G... address
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
        async move { ledger.sign_transaction(path, tx).await }
    });
    let approve = tokio::task::spawn(approve_tx_signature(ui_host_port));

    let result = sign.await.unwrap();
    let _ = approve.await.unwrap();

    match result {
        Ok(response) => {
            assert_eq!( hex::encode(response), "5c2f8eb41e11ab922800071990a25cf9713cc6e7c43e50e0780ddc4c0c6da50c784609ef14c528a12f520d8ea9343b49083f59c51e3f28af8c62b3edeaade60e");
        }
        Err(e) => {
            node.stop();
            println!("{e}");
            assert!(false);
        }
    };

    node.stop();
}

#[tokio::test]
async fn test_sign_tx_hash_when_hash_signing_is_not_enabled() {
    //when hash signing isn't enabled on the device we expect an error
    let docker = clients::Cli::default();
    let node = docker.run(Speculos::new());
    let host_port = node.get_host_port_ipv4(9998);
    let ui_host_port: u16 = node.get_host_port_ipv4(5000);

    wait_for_emulator_start_text(ui_host_port).await;
    // wait for emulator to load  - check the events
    // sleep to account for key delay
    // for some things, waiting for the screen to change... but prob dont need that for this

    let transport = get_zemu_transport("127.0.0.1", host_port).unwrap();
    let ledger_options = Some(LedgerOptions {
        exchange: transport,
        hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
    });
    let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options);

    let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();
    let test_hash = "3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889".as_bytes();

    let result = ledger.sign_transaction_hash(path, test_hash.into()).await;
    if let Err(LedgerError::APDUExchangeError(msg)) = result {
        assert_eq!(msg, "Ledger APDU retcode: 0x6C66");
        // this error code is SW_TX_HASH_SIGNING_MODE_NOT_ENABLED https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
    } else {
        node.stop();
        panic!("Unexpected result: {:?}", result);
    }

    node.stop();
}

#[tokio::test]
async fn test_sign_tx_hash_when_hash_signing_is_enabled() {
    //when hash signing isnt enabled on the device we expect an error
    let docker = clients::Cli::default();
    let node = docker.run(Speculos::new());
    let host_port = node.get_host_port_ipv4(9998);
    let ui_host_port: u16 = node.get_host_port_ipv4(5000);

    wait_for_emulator_start_text(ui_host_port).await;
    enable_hash_signing(ui_host_port).await;

    let transport = get_zemu_transport("127.0.0.1", host_port).unwrap();
    let ledger_options = Some(LedgerOptions {
        exchange: transport,
        hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
    });
    let ledger = Arc::new(LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options));

    let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();
    let mut test_hash = vec![0u8; 32];

    match hex::decode_to_slice(
        "3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889",
        &mut test_hash as &mut [u8],
    ) {
        Ok(()) => {}
        Err(e) => {
            node.stop();
            panic!("Unexpected result: {e}");
        }
    }

    let sign = tokio::task::spawn({
        let ledger = Arc::clone(&ledger);
        async move { ledger.sign_transaction_hash(path, test_hash).await }
    });
    let approve = tokio::task::spawn(approve_tx_hash_signature(ui_host_port));

    let result = sign.await.unwrap();
    let _ = approve.await.unwrap();

    match result {
        Ok(result) => {
            println!("this is the response from signing the hash: {result:?}");
        }
        Err(e) => {
            node.stop();
            panic!("Unexpected result: {e}");
        }
    }

    node.stop();
}

// Based on the zemu click fn
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
    println!("enabling hash signing on the device");

    // right button press
    click(ui_host_port, "button/right").await;

    // both button press
    click(ui_host_port, "button/both").await;

    // both button press
    click(ui_host_port, "button/both").await;

    // right button press
    click(ui_host_port, "button/right").await;

    // right button press
    click(ui_host_port, "button/right").await;

    // both button press
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

async fn wait_for_emulator_start_text(ui_host_port: u16) {
    sleep(Duration::from_secs(1)).await;

    let mut ready = false;
    while !ready {
        let events = get_emulator_events(ui_host_port).await;

        if events.iter().any(|event| event.text == "is ready") {
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

async fn approve_tx_hash_signature(ui_host_port: u16) {
    println!("approving tx hash sig on the device");
    // press the right button 10 times
    for _ in 0..10 {
        click(ui_host_port, "button/right").await;
    }

    // press both buttons
    click(ui_host_port, "button/both").await;
}

async fn approve_tx_signature(ui_host_port: u16) {
    println!("approving tx on the device");
    let mut map = HashMap::new();
    map.insert("action", "press-and-release");

    // press right button 17 times
    let client = reqwest::Client::new();
    for _ in 0..17 {
        client
            .post(format!("http://localhost:{ui_host_port}/button/right"))
            .json(&map)
            .send()
            .await
            .unwrap();
    }

    // press both buttons
    client
        .post(format!("http://localhost:{ui_host_port}/button/both"))
        .json(&map)
        .send()
        .await
        .unwrap();
}
