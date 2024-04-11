// https://github.com/zondax/ledger-rs

use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};

use soroban_env_host::xdr::{
    self, AccountId, DecoratedSignature, Hash, HashIdPreimage, HashIdPreimageSorobanAuthorization,
    InvokeHostFunctionOp, Limits, Operation, OperationBody, PublicKey, ScAddress, ScMap, ScSymbol,
    ScVal, Signature, SignatureHint, SorobanAddressCredentials, SorobanAuthorizationEntry,
    SorobanAuthorizedFunction, SorobanCredentials, Transaction, TransactionEnvelope,
    TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
    TransactionV1Envelope, Uint256, WriteXdr,
};

pub mod app;

mod emulator;

mod docker;

mod transport_zemu_http;

use crate::app::get_zemu_transport;
use crate::{app::new_get_transport, emulator::Emulator};
enum Error {}

#[cfg(test)]
mod test {

    use std::{collections::HashMap, path::PathBuf, str::FromStr, thread, time::Duration};

    use super::*;

    use once_cell::sync::Lazy;
    use serial_test::serial;

    use stellar_xdr::curr::{
        HostFunction, InvokeContractArgs, Memo, MuxedAccount, PaymentOp, Preconditions,
        SequenceNumber, StringM, TransactionExt, TransactionV0, TransactionV0Ext, VecM,
    };

    use tokio::time::sleep;

    use crate::app::LedgerError::APDUExchangeError;

    #[ignore]
    #[tokio::test]
    #[serial]
    async fn test_get_public_key_with_ledger_device() {
        let transport = new_get_transport().unwrap();
        let ledger = app::Ledger::new(transport);
        let public_key = ledger.get_public_key(0).await;
        println!("{public_key:?}");
        assert!(public_key.is_ok());
    }

    #[tokio::test]
    async fn test_get_public_key() {
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
        let ledger = app::Ledger::new(transport);

        match ledger.get_public_key(0).await {
            Ok(public_key) => {
                let public_key_string = public_key.to_string();
                // This is determined by the seed phrase used to start up the emulator
                // TODO: make the seed phrase configurable
                let expected_public_key =
                    "GDUTHCF37UX32EMANXIL2WOOVEDZ47GHBTT3DYKU6EKM37SOIZXM2FN7";
                assert_eq!(public_key_string, expected_public_key);
            }
            Err(e) => {
                println!("{e}");
                assert!(false);
                stop_emulator(&mut emulator).await;
            }
        }

        stop_emulator(&mut emulator).await;
    }

    #[tokio::test]
    async fn test_get_app_configuration() {
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
        let ledger = app::Ledger::new(transport);

        match ledger.get_app_configuration().await {
            Ok(config) => {
                assert_eq!(config, vec![0, 5, 0, 3]);
            }
            Err(e) => {
                println!("{e}");
                assert!(false);
                stop_emulator(&mut emulator).await;
            }
        };

        stop_emulator(&mut emulator).await;
    }

    #[tokio::test]
    async fn test_sign_tx() {
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
        let ledger = app::Ledger::new(transport);

        let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();

        // this transaction came from https://github.com/stellar/rs-stellar-xdr/blob/main/tests/tx_small.rs
        // and i am getting a retcode of 27684 which is unknown op
        // built this tx with https://laboratory.stellar.org/#xdr-viewer?input=AAAAAgAAAAAg2pmLdeQrH3%2BF0HXBJ%2FWyRt8SrZbwELz3929ysW5XEwAAAGQAAAAAAAAAAQAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAAQAAAACVQfAnRiQMHp84Q9KOVvClg%2BzSdQL7D0on1NCSL%2BBkogAAAAAAAAAAAJiWgAAAAAAAAAAA&type=TransactionEnvelope

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

        match ledger.sign_transaction(path, tx).await {
            Ok(response) => {
                assert_eq!( hex::encode(response), "77b9f6e6924e5db2e35c5ecd7dd95248eadd51ea35d61e467cf6ba0df28ca7f38674e3fea8c8a3e2a0fa45f49d4381f9cf24bcc0ff8b708c9337beb854e98e0d");
            }
            Err(e) => {
                stop_emulator(&mut emulator).await;
                println!("{e}");
                assert!(false);
            }
        };

        stop_emulator(&mut emulator).await;
    }

    #[tokio::test]
    async fn test_sign_tx_hash_when_hash_signing_is_not_enabled() {
        //when hash signing isnt enabled on the device we expect an error
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
        let ledger = app::Ledger::new(transport);

        let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();
        let test_hash =
            "3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889".as_bytes();

        let result = ledger.sign_transaction_hash(path, test_hash.into()).await;
        if let Err(APDUExchangeError(msg)) = result {
            assert_eq!(msg, "Ledger APDU retcode: 0x6C66");
            // this error code is SW_TX_HASH_SIGNING_MODE_NOT_ENABLED https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
        } else {
            stop_emulator(&mut emulator).await;
            panic!("Unexpected result: {:?}", result);
        }

        stop_emulator(&mut emulator).await;
    }

    #[tokio::test]
    async fn test_sign_tx_hash_when_hash_signing_is_enabled() {
        //when hash signing isnt enabled on the device we expect an error
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;
        enable_hash_signing().await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
        let ledger = app::Ledger::new(transport);

        let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();
        let mut test_hash = vec![0u8; 32];

        match hex::decode_to_slice(
            "3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889",
            &mut test_hash as &mut [u8],
        ) {
            Ok(()) => {}
            Err(e) => {
                stop_emulator(&mut emulator).await;
                panic!("Unexpected result: {e}");
            }
        }

        let result = ledger.sign_transaction_hash(path, test_hash);

        approve_tx_hash_signature().await;

        match result.await {
            Ok(result) => {
                println!("this is the response from signing the hash: {result:?}");
            }
            Err(e) => {
                stop_emulator(&mut emulator).await;
                panic!("Unexpected result: {e}");
            }
        }

        stop_emulator(&mut emulator).await;
    }

    async fn start_emulator(e: &mut Emulator) {
        let start_result = e.run().await;
        assert!(start_result.is_ok());

        //TODO: handle this in a different way
        // perhaps i can check the endpoint to see if its up before trying to send anything
        sleep(Duration::from_secs(2)).await;
    }

    async fn stop_emulator(e: &mut Emulator) {
        let stop_result = e.stop().await;
        assert!(stop_result.is_ok());
    }

    // FIXME lol/sob
    async fn enable_hash_signing() {
        // let client = reqwest::Client::new();
        // client.post("http://localhost:5001/button/right")
        let mut map = HashMap::new();
        map.insert("action", "press-and-release");

        let client = reqwest::Client::new();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();

        // both button press
        client
            .post("http://localhost:5001/button/both")
            .json(&map)
            .send()
            .await
            .unwrap();

        // both button press
        client
            .post("http://localhost:5001/button/both")
            .json(&map)
            .send()
            .await
            .unwrap();

        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();

        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();

        // both button press
        client
            .post("http://localhost:5001/button/both")
            .json(&map)
            .send()
            .await
            .unwrap();
    }

    async fn approve_tx_hash_signature() {
        println!("approving tx hash sig");

        // let client = reqwest::Client::new();
        // client.post("http://localhost:5001/button/right")
        let mut map = HashMap::new();
        map.insert("action", "press-and-release");

        let client = reqwest::Client::new();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();

        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();
        // right button press
        client
            .post("http://localhost:5001/button/right")
            .json(&map)
            .send()
            .await
            .unwrap();
        // both button press
        client
            .post("http://localhost:5001/button/both")
            .json(&map)
            .send()
            .await
            .unwrap();
    }
}
