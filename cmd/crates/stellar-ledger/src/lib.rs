use futures::executor::block_on;
use ledger_transport::{APDUCommand, Exchange};
use ledger_transport_hid::{
    hidapi::{HidApi, HidError},
    LedgerHIDError, TransportNativeHID,
};
use sha2::{Digest, Sha256};

use soroban_env_host::xdr::{Hash, Transaction};
use std::vec;
use stellar_strkey::DecodeError;
use stellar_xdr::curr::{
    DecoratedSignature, Error as XdrError, Limits, Signature, SignatureHint, TransactionEnvelope,
    TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
    TransactionV1Envelope, WriteXdr,
};

pub use crate::signer::{Error, Stellar};

mod emulator_http_transport;
mod signer;
mod speculos;

#[cfg(all(test, feature = "emulator-tests"))]
mod emulator_tests;

// this is from https://github.com/LedgerHQ/ledger-live/blob/36cfbf3fa3300fd99bcee2ab72e1fd8f280e6280/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L181
const APDU_MAX_SIZE: u8 = 150;
const HD_PATH_ELEMENTS_COUNT: u8 = 3;
const BUFFER_SIZE: u8 = 1 + HD_PATH_ELEMENTS_COUNT * 4;
const CHUNK_SIZE: u8 = APDU_MAX_SIZE - BUFFER_SIZE;

// These constant values are from https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
const SIGN_TX_RESPONSE_SIZE: usize = 64;

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

    #[error("Error with ADPU exchange with Ledger device: {0}")]
    APDUExchangeError(String),

    #[error("Error occurred while exchanging with Ledger device: {0}")]
    LedgerConnectionError(String),

    #[error("Error occurred while parsing BIP32 path: {0}")]
    Bip32PathError(String),

    #[error(transparent)]
    XdrError(#[from] XdrError),

    #[error(transparent)]
    DecodeError(#[from] DecodeError),
}

pub struct LedgerOptions<T: Exchange> {
    exchange: T,
    hd_path: slip10::BIP32Path,
}

pub struct LedgerSigner<T: Exchange> {
    network_passphrase: String,
    transport: T,
    pub hd_path: slip10::BIP32Path,
}

pub fn native_signer(network_passphrase: &str, hd_path: u32) -> Result<NativeSigner, LedgerError> {
    NativeSigner::new(network_passphrase, hd_path)
}

pub struct NativeSigner(LedgerSigner<TransportNativeHID>);

impl AsRef<LedgerSigner<TransportNativeHID>> for NativeSigner {
    fn as_ref(&self) -> &LedgerSigner<TransportNativeHID> {
        &self.0
    }
}

impl NativeSigner {
    pub fn new(network_passphrase: &str, hd_path: u32) -> Result<Self, LedgerError> {
        Ok(Self(LedgerSigner {
            network_passphrase: network_passphrase.to_string(),
            transport: get_transport()?,
            hd_path: bip_path_from_index(hd_path)?,
        }))
    }
}

impl<T> LedgerSigner<T>
where
    T: Exchange,
{
    /// Get the public key from the device
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or getting the public key from the device
    pub async fn get_public_key(
        &self,
        index: u32,
    ) -> Result<stellar_strkey::ed25519::PublicKey, LedgerError> {
        let hd_path = bip_path_from_index(index)?;
        Self::get_public_key_with_display_flag(self, hd_path, false).await
    }
    /// Synchronous version of `get_public_key`
    /// # Errors
    ///
    pub fn get_public_key_sync(
        &self,
        index: u32,
    ) -> Result<stellar_strkey::ed25519::PublicKey, LedgerError> {
        block_on(self.get_public_key(index))
    }

    /// Get the device app's configuration
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or getting the config from the device
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

    /// Sign a Stellar transaction hash with the account on the Ledger device
    /// based on impl from [https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L166](https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L166)
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or signing the given tx on the device. Or, if the device has not enabled hash signing
    pub async fn sign_blob(
        &self,
        hd_path: slip10::BIP32Path,
        blob: &[u8],
    ) -> Result<Vec<u8>, LedgerError> {
        let mut hd_path_to_bytes = hd_path_to_bytes(&hd_path)?;

        let capacity = 1 + hd_path_to_bytes.len() + blob.len();
        let mut data: Vec<u8> = Vec::with_capacity(capacity);

        data.insert(0, HD_PATH_ELEMENTS_COUNT);
        data.append(&mut hd_path_to_bytes);
        data.extend_from_slice(blob);

        let command = APDUCommand {
            cla: CLA,
            ins: SIGN_TX_HASH,
            p1: P1_SIGN_TX_HASH,
            p2: P2_SIGN_TX_HASH,
            data,
        };

        self.send_command_to_ledger(command).await
    }

    /// Sign a Stellar transaction hash with the account on the Ledger device
    /// based on impl from [https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L166](https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L166)
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or signing the given tx on the device. Or, if the device has not enabled hash signing
    pub async fn sign_transaction_hash(
        &self,
        hd_path: slip10::BIP32Path,
        transaction_hash: &[u8],
    ) -> Result<Vec<u8>, LedgerError> {
        self.sign_blob(hd_path, transaction_hash).await
    }

    /// Sign a Stellar transaction with the account on the Ledger device
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or signing the given tx on the device
    #[allow(clippy::missing_panics_doc)]
    async fn sign_transaction(
        &self,
        hd_path: slip10::BIP32Path,
        transaction: Transaction,
    ) -> Result<Vec<u8>, LedgerError> {
        let tagged_transaction =
            TransactionSignaturePayloadTaggedTransaction::Tx(transaction.clone());
        let network_hash = self.network_hash();
        let signature_payload = TransactionSignaturePayload {
            network_id: network_hash,
            tagged_transaction,
        };
        let mut signature_payload_as_bytes = signature_payload.to_xdr(Limits::none())?;

        let mut hd_path_to_bytes = hd_path_to_bytes(&hd_path)?;

        let capacity = 1 + hd_path_to_bytes.len() + signature_payload_as_bytes.len();
        let mut data: Vec<u8> = Vec::with_capacity(capacity);

        data.insert(0, HD_PATH_ELEMENTS_COUNT);
        data.append(&mut hd_path_to_bytes);
        data.append(&mut signature_payload_as_bytes);

        let chunks = data.chunks(CHUNK_SIZE as usize);
        let chunks_count = chunks.len();

        let mut result = Vec::with_capacity(SIGN_TX_RESPONSE_SIZE);
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

    /// The `display_and_confirm` bool determines if the Ledger will display the public key on its screen and requires user approval to share
    async fn get_public_key_with_display_flag(
        &self,
        hd_path: slip10::BIP32Path,
        display_and_confirm: bool,
    ) -> Result<stellar_strkey::ed25519::PublicKey, LedgerError> {
        // convert the hd_path into bytes to be sent as `data` to the Ledger
        // the first element of the data should be the number of elements in the path
        let mut hd_path_to_bytes = hd_path_to_bytes(&hd_path)?;
        let hd_path_elements_count = hd_path.depth();
        hd_path_to_bytes.insert(0, hd_path_elements_count);

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
            p2,
            data: hd_path_to_bytes,
        };

        tracing::info!("APDU in: {}", hex::encode(command.serialize()));

        match self.send_command_to_ledger(command).await {
            Ok(value) => Ok(stellar_strkey::ed25519::PublicKey::from_payload(&value)?),
            Err(err) => Err(err),
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
                if response.retcode() == RETURN_CODE_OK {
                    return Ok(response.data().to_vec());
                }

                let retcode = response.retcode();
                let error_string = format!("Ledger APDU retcode: 0x{retcode:X}");
                Err(LedgerError::APDUExchangeError(error_string))
            }
            Err(_err) => Err(LedgerError::LedgerConnectionError(
                "Error connecting to ledger device".to_string(),
            )),
        }
    }
}

impl<T: Exchange> Stellar for LedgerSigner<T> {
    type Init = LedgerOptions<T>;

    fn new(network_passphrase: &str, options: Option<LedgerOptions<T>>) -> Self {
        let options_unwrapped = options.expect("LedgerSigner should have LedgerOptions passed in");
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
        let signature = block_on(self.sign_transaction_hash(self.hd_path.clone(), &txn)) //TODO: refactor sign_transaction_hash
            .map_err(|e| {
                tracing::error!("Error signing transaction hash with Ledger device: {e}");
                Error::MissingSignerForAddress {
                    address: source_account.to_string(),
                }
            })?;

        let hint = source_account.to_string().into_bytes()[28..]
            .try_into()
            .map_err(|e| {
                tracing::error!("Error converting source_account to string: {e}");
                Error::MissingSignerForAddress {
                    address: source_account.to_string(),
                }
            })?;
        let sig_bytes = signature.try_into()?;
        Ok(DecoratedSignature {
            hint: SignatureHint(hint),
            signature: Signature(sig_bytes),
        })
    }

    fn sign_txn(
        &self,
        txn: Transaction,
        source_account: &stellar_strkey::Strkey,
    ) -> Result<TransactionEnvelope, Error> {
        let signature = block_on(self.sign_transaction(self.hd_path.clone(), txn.clone()))
            .map_err(|e| {
                tracing::error!("Error signing transaction with Ledger device: {e}");
                Error::MissingSignerForAddress {
                    address: "source_account".to_string(),
                }
            })?;

        let hint = source_account.to_string().into_bytes()[28..]
            .try_into()
            .map_err(|e| {
                tracing::error!("Error converting source_account to string: {e}");
                Error::MissingSignerForAddress {
                    address: source_account.to_string(),
                }
            })?;
        let sig_bytes = signature.try_into()?;
        let decorated_signature = DecoratedSignature {
            hint: SignatureHint(hint),
            signature: Signature(sig_bytes),
        };

        Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: txn,
            signatures: vec![decorated_signature].try_into()?,
        }))
    }
    
    fn sign_blob(&self,
        txn: &[u8],
        source_account: &stellar_strkey::Strkey,
    ) -> Result<Vec<u8>, Error> {
        todo!()
    }
}

fn bip_path_from_index(index: u32) -> Result<slip10::BIP32Path, LedgerError> {
    let path = format!("m/44'/148'/{index}'");
    path.parse().map_err(|e| {
        let error_string = format!("Error parsing BIP32 path: {e}");
        LedgerError::Bip32PathError(error_string)
    })
}

fn hd_path_to_bytes(hd_path: &slip10::BIP32Path) -> Result<Vec<u8>, LedgerError> {
    let hd_path_indices = 0..hd_path.depth();
    let result = hd_path_indices
        .into_iter()
        .map(|index| {
            Ok(hd_path
                .index(index)
                .ok_or_else(|| LedgerError::Bip32PathError(format!("{hd_path}")))?
                .to_be_bytes())
        })
        .collect::<Result<Vec<_>, LedgerError>>()?;
    Ok(result.into_iter().flatten().collect())
}

pub fn get_transport() -> Result<TransportNativeHID, LedgerError> {
    // instantiate the connection to Ledger, this will return an error if Ledger is not connected
    let hidapi = HidApi::new().map_err(LedgerError::HidApiError)?;
    TransportNativeHID::new(&hidapi).map_err(LedgerError::LedgerHidError)
}
#[cfg(test)]
mod test {
    use std::str::FromStr;

    use httpmock::prelude::*;
    use serde_json::json;

    use crate::emulator_http_transport::EmulatorHttpTransport;

    use soroban_env_host::xdr::Transaction;
    use std::vec;

    use crate::signer::Stellar;
    use soroban_env_host::xdr::{self, Operation, OperationBody, Uint256};

    use crate::{LedgerError, LedgerOptions, LedgerSigner};

    use stellar_xdr::curr::{
        Memo, MuxedAccount, PaymentOp, Preconditions, SequenceNumber, TransactionExt,
    };

    const TEST_NETWORK_PASSPHRASE: &str = "Test SDF Network ; September 2015";

    #[tokio::test]
    async fn test_get_public_key() {
        let server = MockServer::start();
        let mock_server = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e00200000d038000002c8000009480000000" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "e93388bbfd2fbd11806dd0bd59cea9079e7cc70ce7b1e154f114cdfe4e466ecd9000"}));
        });

        let transport = EmulatorHttpTransport::new(&server.host(), server.port());
        let ledger_options = LedgerOptions {
            exchange: transport,
            hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
        };

        let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, Some(ledger_options));
        let public_key = ledger.get_public_key(0).await.unwrap();
        let public_key_string = public_key.to_string();
        let expected_public_key = "GDUTHCF37UX32EMANXIL2WOOVEDZ47GHBTT3DYKU6EKM37SOIZXM2FN7";
        assert_eq!(public_key_string, expected_public_key);

        mock_server.assert();
    }

    #[tokio::test]
    async fn test_get_app_configuration() {
        let server = MockServer::start();
        let mock_server = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e006000000" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "000500039000"}));
        });

        let transport = EmulatorHttpTransport::new(&server.host(), server.port());
        let ledger_options = LedgerOptions {
            exchange: transport,
            hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
        };

        let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, Some(ledger_options));
        let config = ledger.get_app_configuration().await.unwrap();
        assert_eq!(config, vec![0, 5, 0, 3]);

        mock_server.assert();
    }

    #[tokio::test]
    async fn test_sign_tx() {
        let server = MockServer::start();
        let mock_request_1 = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e004008089038000002c8000009480000000cee0302d59844d32bdca915c8203dd44b33fbb7edc19051ea37abedf28ecd472000000020000000000000000000000000000000000000000000000000000000000000000000000000000006400000000000000010000000000000001000000075374656c6c6172000000000100000001000000000000000000000000" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "9000"}));
        });

        let mock_request_2 = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e0048000500000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006400000000" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "5c2f8eb41e11ab922800071990a25cf9713cc6e7c43e50e0780ddc4c0c6da50c784609ef14c528a12f520d8ea9343b49083f59c51e3f28af8c62b3edeaade60e9000"}));
        });

        let transport = EmulatorHttpTransport::new(&server.host(), server.port());
        let ledger_options = LedgerOptions {
            exchange: transport,
            hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
        };
        let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, Some(ledger_options));
        let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();

        let fake_source_acct = [0; 32];
        let fake_dest_acct = [0; 32];
        let tx = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(fake_source_acct)),
            fee: 100,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::Text("Stellar".as_bytes().try_into().unwrap()),
            ext: TransactionExt::V0,
            operations: [Operation {
                source_account: Some(MuxedAccount::Ed25519(Uint256(fake_source_acct))),
                body: OperationBody::Payment(PaymentOp {
                    destination: MuxedAccount::Ed25519(Uint256(fake_dest_acct)),
                    asset: xdr::Asset::Native,
                    amount: 100,
                }),
            }]
            .try_into()
            .unwrap(),
        };

        let response = ledger.sign_transaction(path, tx).await.unwrap();
        assert_eq!(
            hex::encode(response),
            "5c2f8eb41e11ab922800071990a25cf9713cc6e7c43e50e0780ddc4c0c6da50c784609ef14c528a12f520d8ea9343b49083f59c51e3f28af8c62b3edeaade60e"
        );

        mock_request_1.assert();
        mock_request_2.assert();
    }

    #[tokio::test]
    async fn test_sign_tx_hash_when_hash_signing_is_not_enabled() {
        let server = MockServer::start();
        let mock_server = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e00800004d038000002c800000948000000033333839653966306631613635663139373336636163663534346332653832353331336538343437663536393233336262386462333961613630376338383839" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "6c66"}));
        });

        let transport = EmulatorHttpTransport::new(&server.host(), server.port());
        let ledger_options = LedgerOptions {
            exchange: transport,
            hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
        };
        let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, Some(ledger_options));

        let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();
        let test_hash = b"3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889";

        let err = ledger
            .sign_transaction_hash(path, test_hash)
            .await
            .unwrap_err();
        if let LedgerError::APDUExchangeError(msg) = err {
            assert_eq!(msg, "Ledger APDU retcode: 0x6C66");
        } else {
            panic!("Unexpected error: {err:?}");
        }

        mock_server.assert();
    }

    #[tokio::test]
    async fn test_sign_tx_hash_when_hash_signing_is_enabled() {
        let server = MockServer::start();
        let mock_server = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e00800002d038000002c80000094800000003389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "6970b9c9d3a6f4de7fb93e8d3920ec704fc4fece411873c40570015bbb1a60a197622bc3bf5644bb38ae73e1b96e4d487d716d142d46c7e944f008dece92df079000"}));
        });

        let transport = EmulatorHttpTransport::new(&server.host(), server.port());
        let ledger_options = LedgerOptions {
            exchange: transport,
            hd_path: slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap(),
        };
        let ledger = LedgerSigner::new(TEST_NETWORK_PASSPHRASE, Some(ledger_options));
        let path = slip10::BIP32Path::from_str("m/44'/148'/0'").unwrap();
        let mut test_hash = vec![0u8; 32];

        hex::decode_to_slice(
            "3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889",
            &mut test_hash as &mut [u8],
        )
        .unwrap();

        let response = ledger
            .sign_transaction_hash(path, &test_hash)
            .await
            .unwrap();

        assert_eq!(
            hex::encode(response),
            "6970b9c9d3a6f4de7fb93e8d3920ec704fc4fece411873c40570015bbb1a60a197622bc3bf5644bb38ae73e1b96e4d487d716d142d46c7e944f008dece92df07"
        );

        mock_server.assert();
    }
}
