mod transport_zemu_http;
use async_trait::async_trait;
use futures::executor::block_on;
use ledger_transport::{APDUCommand, Exchange};
use ledger_transport_hid::{
    hidapi::{HidApi, HidError},
    LedgerHIDError, TransportNativeHID,
};
use sha2::{Digest, Sha256};

use soroban_env_host::xdr::{Hash, Transaction};
use std::vec;
use stellar_xdr::curr::{
    self, DecoratedSignature, Limits, Signature, SignatureHint, TransactionEnvelope,
    TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
    TransactionV1Envelope, WriteXdr,
};

use crate::signer::{Error, Stellar};
use crate::transport_zemu_http::TransportZemuHttp;

mod docker;
mod emulator;
mod signer;

// this is from https://github.com/LedgerHQ/ledger-live/blob/36cfbf3fa3300fd99bcee2ab72e1fd8f280e6280/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L181
const APDU_MAX_SIZE: u8 = 150;

// these constant values are from https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
const CLA: u8 = 0xE0;

const GET_PUBLIC_KEY: u8 = 0x02;
const P1_GET_PUBLIC_KEY: u8 = 0x00;
const P2_GET_PUBLIC_KEY_NO_DISPLAY: u8 = 0x00;
const P2_GET_PUBLIC_KEY_DISPLAY: u8 = 0x01;

const SIGN_TX: u8 = 0x04;
const P1_SIGN_TX_FIRST: u8 = 0x00;
const P1_SIGN_TX_NOT_FIRST: u8 = 0x80;
const P2_SIGN_TX_LAST: u8 = 0x00;
const P2_SIGN_TX_MORE: u8 = 0x80;

const GET_APP_CONFIGURATION: u8 = 0x06;
const P1_GET_APP_CONFIGURATION: u8 = 0x00;
const P2_GET_APP_CONFIGURATION: u8 = 0x00;

const SIGN_TX_HASH: u8 = 0x08;
const P1_SIGN_TX_HASH: u8 = 0x00;
const P2_SIGN_TX_HASH: u8 = 0x00;

const RETURN_CODE_OK: u16 = 36864; // APDUAnswer.retcode which means success from Ledger

#[derive(thiserror::Error, Debug)]
pub enum LedgerError {
    #[error("Error occurred while initializing HIDAPI: {0}")]
    HidApiError(#[from] HidError),

    #[error("Error occurred while initializing Ledger HID transport: {0}")]
    LedgerHidError(#[from] LedgerHIDError),

    #[error("Error with ADPU exchange with Ledger device: {0}")] //TODO update this message
    APDUExchangeError(String),

    #[error("Error occurred while exchanging with Ledger device: {0}")]
    LedgerConnectionError(String),
}

pub struct LedgerOptions<T: Exchange> {
    exchange: T,
    hd_path: slip10::BIP32Path,
}

pub struct LedgerSigner<T: Exchange> {
    network_passphrase: String,
    transport: T,
    hd_path: slip10::BIP32Path,
}

impl<T> LedgerSigner<T>
where
    T: Exchange,
{
    pub async fn get_public_key(
        &self,
        index: u32,
    ) -> Result<stellar_strkey::ed25519::PublicKey, LedgerError> {
        let hd_path = bip_path_from_index(index);
        Self::get_public_key_with_display_flag(self, hd_path, false).await
    }

    pub async fn get_app_configuration(&self) -> Result<Vec<u8>, LedgerError> {
        let command = APDUCommand {
            cla: CLA,
            ins: GET_APP_CONFIGURATION,
            p1: P1_GET_APP_CONFIGURATION,
            p2: P2_GET_APP_CONFIGURATION,
            data: vec![],
        };
        self.send_command_to_ledger(command).await
    }

    // based on impl from https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L166
    async fn sign_transaction_hash(
        &self,
        hd_path: slip10::BIP32Path,
        transaction_hash: Vec<u8>,
    ) -> Result<Vec<u8>, LedgerError> {
        // convert the hd_path into bytes to be sent as `data` to the Ledger
        // the first element of the data should be the number of elements in the path

        let mut hd_path_to_bytes = hd_path_to_bytes(&hd_path);
        let hd_path_elements_count = hd_path.depth();
        hd_path_to_bytes.insert(0, hd_path_elements_count);

        let mut data = hd_path_to_bytes;
        data.append(&mut transaction_hash.clone());

        let command = APDUCommand {
            cla: CLA,
            ins: SIGN_TX_HASH,
            p1: P1_SIGN_TX_HASH,
            p2: P2_SIGN_TX_HASH,
            data: data,
        };

        self.send_command_to_ledger(command).await
    }

    pub async fn sign_transaction(
        &self,
        hd_path: slip10::BIP32Path,
        transaction: Transaction,
    ) -> Result<Vec<u8>, LedgerError> {
        let tagged_transaction =
            TransactionSignaturePayloadTaggedTransaction::Tx(transaction.clone());

        let passphrase = self.network_passphrase.clone();
        let network_hash = Hash(Sha256::digest(passphrase.as_bytes()).into());

        let signature_payload = TransactionSignaturePayload {
            network_id: network_hash,
            tagged_transaction: tagged_transaction,
        };

        let mut signature_payload_as_bytes = signature_payload.to_xdr(Limits::none()).unwrap();

        let mut data: Vec<u8> = Vec::new();

        let mut hd_path_to_bytes = hd_path_to_bytes(&hd_path);
        let hd_path_elements_count = hd_path.depth();

        data.insert(0, hd_path_elements_count);
        data.append(&mut hd_path_to_bytes);
        data.append(&mut signature_payload_as_bytes);

        let buffer_size = 1 + hd_path_elements_count * 4;
        let chunk_size = APDU_MAX_SIZE - buffer_size;

        let chunks = data.chunks(chunk_size as usize);
        let chunks_count = chunks.len();

        let mut result = Vec::new();
        println!("chunks_count: {:?}", chunks_count);

        // notes:
        // the first chunk has the hd_path_elements_count and the hd_path at the beginning, before the tx [3, 128...122...47]
        // the second chunk has just the end of the tx [224, 100... 0, 0, 0, 0]

        for (i, chunk) in chunks.enumerate() {
            let is_first_chunk = i == 0;
            let is_last_chunk = chunks_count == i + 1;

            let command = APDUCommand {
                cla: CLA,
                ins: SIGN_TX,
                p1: if is_first_chunk {
                    P1_SIGN_TX_FIRST
                } else {
                    P1_SIGN_TX_NOT_FIRST
                },
                p2: if is_last_chunk {
                    P2_SIGN_TX_LAST
                } else {
                    P2_SIGN_TX_MORE
                },
                data: chunk.to_vec(),
            };

            match self.send_command_to_ledger(command).await {
                Ok(mut r) => {
                    result.append(&mut r);
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        Ok(result)
    }

    /// The display_and_confirm bool determines if the Ledger will display the public key on its screen and requires user approval to share
    async fn get_public_key_with_display_flag(
        &self,
        hd_path: slip10::BIP32Path,
        display_and_confirm: bool,
    ) -> Result<stellar_strkey::ed25519::PublicKey, LedgerError> {
        // convert the hd_path into bytes to be sent as `data` to the Ledger
        // the first element of the data should be the number of elements in the path
        let mut hd_path_to_bytes = hd_path_to_bytes(&hd_path);
        let hd_path_elements_count = hd_path.depth();
        hd_path_to_bytes.insert(0, hd_path_elements_count);

        println!("data: {:?}", hd_path_to_bytes);

        let p2 = if display_and_confirm {
            P2_GET_PUBLIC_KEY_DISPLAY
        } else {
            P2_GET_PUBLIC_KEY_NO_DISPLAY
        };

        // more information about how to build this command can be found at https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
        let command = APDUCommand {
            cla: CLA,
            ins: GET_PUBLIC_KEY,
            p1: P1_GET_PUBLIC_KEY,
            p2: p2,
            data: hd_path_to_bytes,
        };

        tracing::info!("APDU in: {}", hex::encode(&command.serialize()));

        match self.send_command_to_ledger(command).await {
            Ok(value) => {
                return Ok(stellar_strkey::ed25519::PublicKey::from_payload(&value).unwrap());
            }
            Err(err) => {
                return Err(err);
            }
        }
    }

    async fn send_command_to_ledger(
        &self,
        command: APDUCommand<Vec<u8>>,
    ) -> Result<Vec<u8>, LedgerError> {
        match self.transport.exchange(&command).await {
            Ok(response) => {
                tracing::info!(
                    "APDU out: {}\nAPDU ret code: {:x}",
                    hex::encode(response.apdu_data()),
                    response.retcode(),
                );
                // Ok means we successfully connected with the Ledger but it doesn't mean our request succeeded. We still need to check the response.retcode
                println!("RETCODE: {:?}", response.retcode());
                println!("response: {:?}", response.data());
                if response.retcode() == RETURN_CODE_OK {
                    return Ok(response.data().to_vec());
                } else {
                    let retcode = response.retcode();

                    let error_string = format!("Ledger APDU retcode: 0x{:X}", retcode);
                    return Err(LedgerError::APDUExchangeError(error_string));
                }
            }
            Err(err) => {
                //FIX ME!!!!
                return Err(LedgerError::LedgerConnectionError("test".to_string()));
            }
        };
    }
}

#[async_trait]
impl<T: Exchange> Stellar for LedgerSigner<T> {
    type Init = LedgerOptions<T>;

    fn new(network_passphrase: &str, options: Option<LedgerOptions<T>>) -> Self {
        let options_unwrapped = options.unwrap();
        LedgerSigner {
            network_passphrase: network_passphrase.to_string(),
            transport: options_unwrapped.exchange,
            hd_path: options_unwrapped.hd_path,
        }
    }

    fn network_hash(&self) -> stellar_xdr::curr::Hash {
        Hash(Sha256::digest(self.network_passphrase.as_bytes()).into())
    }

    fn sign_txn_hash(
        &self,
        txn: [u8; 32],
        source_account: &stellar_strkey::Strkey,
    ) -> Result<DecoratedSignature, Error> {
        let signature = block_on(self.sign_transaction_hash(self.hd_path.clone(), txn.to_vec())) //TODO: refactor sign_transaction_hash
            .unwrap(); // FIXME: handle error

        let sig_bytes = signature.try_into().unwrap(); // FIXME: handle error
        Ok(DecoratedSignature {
            hint: SignatureHint([0u8; 4]), //FIXME
            signature: Signature(sig_bytes),
        })
    }

    fn sign_txn(
        &self,
        txn: Transaction,
        source_account: &stellar_strkey::Strkey,
    ) -> Result<TransactionEnvelope, Error> {
        let signature = block_on(self.sign_transaction(self.hd_path.clone(), txn.clone())).unwrap(); // FIXME: handle error

        let sig_bytes = signature.try_into().unwrap(); // FIXME: handle error
        let decorated_signature = DecoratedSignature {
            hint: SignatureHint([0u8; 4]), //FIXME
            signature: Signature(sig_bytes),
        };

        Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: txn,
            signatures: vec![decorated_signature].try_into().unwrap(), //fixme: remove unwrap
        }))
    }
}

fn bip_path_from_index(index: u32) -> slip10::BIP32Path {
    let path = format!("m/44'/148'/{index}'");
    path.parse().unwrap() // this is basically the same thing as slip10::BIP32Path::from_str

    // the device handles this part: https://github.com/AhaLabs/rs-sep5/blob/9d6e3886b4b424dd7b730ec24c865f6fad5d770c/src/seed_phrase.rs#L86
}

fn hd_path_to_bytes(hd_path: &slip10::BIP32Path) -> Vec<u8> {
    println!("hd_path.depth: {:?}", hd_path.depth());
    (0..hd_path.depth())
        .map(|index| {
            let value = *hd_path.index(index).unwrap();
            value.to_be_bytes()
        })
        .flatten()
        .collect::<Vec<u8>>()
}

pub fn get_transport() -> Result<impl Exchange, LedgerError> {
    // instantiate the connection to Ledger, this will return an error if Ledger is not connected
    let hidapi = HidApi::new().map_err(LedgerError::HidApiError)?;
    TransportNativeHID::new(&hidapi).map_err(LedgerError::LedgerHidError)
}

pub fn get_zemu_transport(host: &str, port: u16) -> Result<impl Exchange, LedgerError> {
    Ok(TransportZemuHttp::new(host, port))
}

#[cfg(test)]
mod test {
    use soroban_env_host::xdr::{self, Operation, OperationBody, Transaction, Uint256};

    use crate::emulator::Emulator;

    use std::{collections::HashMap, str::FromStr, time::Duration};

    use super::*;

    use serial_test::serial;

    use stellar_xdr::curr::{
        Memo, MuxedAccount, PaymentOp, Preconditions, SequenceNumber, TransactionExt,
    };

    use tokio::time::sleep;

    const TEST_NETWORK_PASSPHRASE: &str = "Test SDF Network ; September 2015";

    #[ignore]
    #[tokio::test]
    #[serial]
    async fn test_get_public_key_with_ledger_device() {
        let transport = get_transport().unwrap();
        let ledger_options = Some(LedgerOptions {
            exchange: transport,
            hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
        });
        let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options);
        let public_key = ledger.get_public_key(0).await;
        println!("{public_key:?}");
        assert!(public_key.is_ok());
    }

    #[tokio::test]
    async fn test_get_public_key() {
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
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

    #[ignore]
    #[tokio::test]
    async fn test_get_app_configuration() {
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
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
                println!("{e}");
                assert!(false);
                stop_emulator(&mut emulator).await;
            }
        };

        stop_emulator(&mut emulator).await;
    }

    #[ignore]
    #[tokio::test]
    async fn test_sign_tx() {
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
        let ledger_options = Some(LedgerOptions {
            exchange: transport,
            hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
        });
        let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options);

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

        match ledger.sign_transaction(path, tx).await {
            Ok(response) => {
                assert_eq!( hex::encode(response), "5c2f8eb41e11ab922800071990a25cf9713cc6e7c43e50e0780ddc4c0c6da50c784609ef14c528a12f520d8ea9343b49083f59c51e3f28af8c62b3edeaade60e");
            }
            Err(e) => {
                stop_emulator(&mut emulator).await;
                println!("{e}");
                assert!(false);
            }
        };

        stop_emulator(&mut emulator).await;
    }

    #[ignore]
    #[tokio::test]
    async fn test_sign_tx_hash_when_hash_signing_is_not_enabled() {
        //when hash signing isn't enabled on the device we expect an error
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
        let ledger_options = Some(LedgerOptions {
            exchange: transport,
            hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
        });
        let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options);

        let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();
        let test_hash =
            "3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889".as_bytes();

        let result = ledger.sign_transaction_hash(path, test_hash.into()).await;
        if let Err(LedgerError::APDUExchangeError(msg)) = result {
            assert_eq!(msg, "Ledger APDU retcode: 0x6C66");
            // this error code is SW_TX_HASH_SIGNING_MODE_NOT_ENABLED https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
        } else {
            stop_emulator(&mut emulator).await;
            panic!("Unexpected result: {:?}", result);
        }

        stop_emulator(&mut emulator).await;
    }

    #[ignore]
    #[tokio::test]
    async fn test_sign_tx_hash_when_hash_signing_is_enabled() {
        //when hash signing isnt enabled on the device we expect an error
        let mut emulator = Emulator::new().await;
        start_emulator(&mut emulator).await;
        enable_hash_signing().await;

        let transport = get_zemu_transport("127.0.0.1", 9998).unwrap();
        let ledger_options = Some(LedgerOptions {
            exchange: transport,
            hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
        });
        let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, ledger_options);

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
