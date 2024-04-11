use byteorder::{BigEndian, WriteBytesExt};
use reqwest::Response;
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Write},
    str::FromStr,
    thread::sleep,
    time::Duration,
    vec,
};
use stellar_xdr::curr::{
    self, Hash, Limited, Limits, ReadXdr, TransactionEnvelope, TransactionSignaturePayload,
    TransactionSignaturePayloadTaggedTransaction, TransactionV0, TransactionV0Envelope,
    TransactionV1Envelope, WriteXdr,
};

use ledger_transport::{APDUCommand, Exchange};
use ledger_transport_hid::{
    hidapi::{HidApi, HidError},
    LedgerHIDError, TransportNativeHID,
};

use soroban_env_host::xdr::Transaction;

use crate::transport_zemu_http::{LedgerZemuError, TransportZemuHttp};

const APDU_MAX_SIZE: u8 = 150; // from https://github.com/LedgerHQ/ledger-live/blob/36cfbf3fa3300fd99bcee2ab72e1fd8f280e6280/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L181

// these came from https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
const CLA: u8 = 0xE0; // Instruction class

const GET_PUBLIC_KEY: u8 = 0x02; // Instruction code to get public key
const P1_GET_PUBLIC_KEY: u8 = 0x00;
const P2_GET_PUBLIC_KEY_NO_DISPLAY: u8 = 0x00;
const P2_GET_PUBLIC_KEY_DISPLAY: u8 = 0x01;

const SIGN_TX: u8 = 0x04;
const P1_SIGN_TX_FIRST: u8 = 0x00; // 0
const P1_SIGN_TX_NOT_FIRST: u8 = 0x80; // 128
const P2_SIGN_TX_LAST: u8 = 0x00; // 0
const P2_SIGN_TX_MORE: u8 = 0x80; // 128

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

pub struct Ledger<T> {
    transport: T,
}

impl<T> Ledger<T>
where
    T: Exchange,
{
    pub fn new(transport: T) -> Ledger<T> {
        Ledger {
            transport: transport,
        }
    }

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
    pub async fn sign_transaction_hash(
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
        // FIXME
        let testnet_passphrase = "Test SDF Network ; September 2015";
        let network_hash = Hash(Sha256::digest(testnet_passphrase.as_bytes()).into());

        let signature_payload = TransactionSignaturePayload {
            network_id: network_hash,
            tagged_transaction: tagged_transaction,
        };

        let mut signature_payload_as_bytes = signature_payload.to_xdr(Limits::none()).unwrap();

        // let tx_env = TransactionEnvelope::TxV0(TransactionV0Envelope {
        //     tx: &transaction,
        //     signatures: vec![].try_into().unwrap(),
        // });

        // let tx_as_bytes = &transaction.to_xdr(Limits::none()).unwrap();
        // // let mut tx_env_as_xdr = tx_env.to_xdr(Limits::none()).unwrap();

        // match TransactionV0::from_xdr(tx_as_bytes, Limits::none()) {
        //     Ok(tx) => {
        //         println!("tx: {:?}", tx);
        //     }
        //     Err(e) => {
        //         println!("error: {:?}", e);
        //     }
        // }

        // the test tx from // https://github.com/LedgerHQ/ledger-live/blob/bd5188b5368849cccab74c11cbc64870bd5edbcd/libs/ledgerjs/packages/hw-app-str/tests/Str.test.ts#L47 works. I converted the hex into a byte vec in js:
        // const transaction = Buffer.from(
        //     "7ac33997544e3175d266bd022439b22cdb16508c01163f26e5cb2a3e1045a979000000020000000020da998b75e42b1f7f85d075c127f5b246df12ad96f010bcf7f76f72b16e57130000006400c5b4a5000000190000000000000000000000010000000000000001000000009541f02746240c1e9f3843d28e56f0a583ecd27502fb0f4a27d4d0922fe064a200000000000000000098968000000000",
        //         "hex"
        //       );

        // console.dir(transaction, {'maxArrayLength': 200});

        // which is this byte vec here:

        // let mut tx_as_bytes: Vec<u8> = [
        //     122, 195, 57, 151, 84, 78, 49, 117, 210, 102, 189, 2, 36, 57, 178, 44, 219, 22, 80,
        //     140, 1, 22, 63, 38, 229, 203, 42, 62, 16, 69, 169, 121, 0, 0, 0, 2, 0, 0, 0, 0, 32,
        //     218, 153, 139, 117, 228, 43, 31, 127, 133, 208, 117, 193, 39, 245, 178, 70, 223, 18,
        //     173, 150, 240, 16, 188, 247, 247, 111, 114, 177, 110, 87, 19, 0, 0, 0, 100, 0, 197,
        //     180, 165, 0, 0, 0, 25, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0,
        //     0, 0, 0, 149, 65, 240, 39, 70, 36, 12, 30, 159, 56, 67, 210, 142, 86, 240, 165, 131,
        //     236, 210, 117, 2, 251, 15, 74, 39, 212, 208, 146, 47, 224, 100, 162, 0, 0, 0, 0, 0, 0,
        //     0, 0, 0, 152, 150, 128, 0, 0, 0, 0,
        // ]
        // .to_vec();

        //this is from js-stellar-base tests somewhere (transaction builder test) AHHH THIS ONE WORKED!!! but also seems to be v1 :thinkin:
        // i bet that `signatureBase` is doing something that I need to bake in here somewhere.

        // ah it makes a sig payload... i think that we have that in signer too. i just didnt realize how to put the two things togheter
        let mut tx_as_bytes: Vec<u8> = [
            206, 224, 48, 45, 89, 132, 77, 50, 189, 202, 145, 92, 130, 3, 221, 68, 179, 63, 187,
            126, 220, 25, 5, 30, 163, 122, 190, 223, 40, 236, 212, 114, 0, 0, 0, 2, 0, 0, 0, 0,
            137, 155, 40, 64, 237, 86, 54, 197, 109, 220, 95, 20, 178, 57, 117, 247, 159, 27, 162,
            56, 141, 38, 148, 228, 197, 110, 205, 221, 201, 96, 229, 239, 0, 0, 0, 100, 0, 0, 0, 0,
            0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0,
            0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 210, 152, 197,
            129, 87, 17, 101, 145, 233, 50, 9, 233, 92, 122, 91, 255, 18, 27, 46, 29, 63, 97, 139,
            104, 208, 252, 200, 239, 229, 229, 228, 112, 0, 0, 0, 0, 0, 0, 0, 2, 84, 11, 228, 0, 0,
            0, 0, 0,
        ]
        .to_vec();

        // from what i can tell, this tx does this:
        // max fee: 0.00001 XLM
        // tx source: GAQNVGMLOXSCWH37QXIHLQJH6WZENXYSVWLPAEF4673W64VRNZLRHMFM
        // send 1xlm
        // tx destination:GCKUD4BHIYSAYHU7HBB5FDSW6CSYH3GSOUBPWD2KE7KNBERP4BSKEJDV

        // i feel like i replicated that, but when i try to send the Transaction that is passed into this function, i keep getting back 0x6C24	SW_UNKNOWN_OP	Unknown Stellar operation
        // i wish i could figure out how to decode that tx into a Transaction, so i could see what the heck it is doing differently

        // i wonder if speculos is using a specific xdr version, or maybe its not speculos but the elf file that i created... somehow

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
        let response = self.transport.exchange(&command).await;
        println!("SLEEPING for 10...");
        sleep(Duration::from_secs(10));
        // this is when the user is supposed to confirm the transaction on the Ledger
        // how do i do this programatically with the emulator?
        println!("sleep over, checking the response");

        match response {
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

pub fn new_get_transport() -> Result<impl Exchange, LedgerError> {
    // instantiate the connection to Ledger, this will return an error if Ledger is not connected
    let hidapi = HidApi::new().map_err(LedgerError::HidApiError)?;
    TransportNativeHID::new(&hidapi).map_err(LedgerError::LedgerHidError)
}

pub fn get_zemu_transport(host: &str, port: u16) -> Result<impl Exchange, LedgerError> {
    Ok(TransportZemuHttp::new(host, port))
}
