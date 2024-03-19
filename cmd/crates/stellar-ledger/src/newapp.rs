use byteorder::{BigEndian, WriteBytesExt};
use std::{io::Write, str::FromStr};

use ledger_transport::APDUCommand;
use ledger_transport_hid::{
    hidapi::{HidApi, HidError},
    LedgerHIDError, TransportNativeHID,
};
use slip10::BIP32Path;

// https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
const CLA: u8 = 0xE0; // Instruction class
const GET_PUBLIC_KEY: u8 = 0x02; // Instruction code to get public key

const P2_GET_PUB_NO_DISPLAY: u8 = 0x00;
const P2_GET_PUB_DISPLAY: u8 = 0x01;

const RETURN_CODE_OK: u16 = 36864; // APDUAnswer.retcode which means success from Ledger

#[derive(Debug)]
pub enum LedgerError {
    /// Error occuring on init of hidapid and getting current devices list
    HidApiError(HidError),
    /// Error occuring on creating a new hid transport, connecting to first ledger device found  
    LedgerHidError(LedgerHIDError),
    /// Error occurred while exchanging with Ledger device
    APDUExchangeError(String),
    /// Error with transport
    LedgerHIDError(LedgerHIDError),
}

// this method should mimic the behavior of the splitPath function in the JS fn in https://github.com/LedgerHQ/ledger-live/blob/6f033e6b13ae1bcd960fb1cd041687fd6d0de21b/libs/ledgerjs/packages/hw-app-str/src/utils.ts#L21
fn split_path(path: &str) -> Vec<u32> {
    let mut result: Vec<u32> = Vec::new();

    for component in path.split('/') {
        // first check if the component has a digit in it
        let has_a_number = component.chars().any(|c| c.is_digit(10));

        // if it doesn't have a number (like m), skip it
        if !has_a_number {
            continue;
        }

        // if it does have a number, remove any ' and parse it into an u32
        let component_without_quote = component.replace("'", "");
        if let Ok(mut number) = component_without_quote.parse::<u32>() {
            if component.len() > 1 && component.chars().last() == Some('\'') {
                number += 0x8000_0000;
            }
            result.push(number);
        }
    }

    result
}

pub fn get_public_key(index: u32) -> Result<stellar_strkey::ed25519::PublicKey, LedgerError> {
    let hd_path = bip_from_index(index); // this is the same result as BIP32Path::from_str("m/44'/148'/0'")
    get_public_key_with_display_flag(hd_path, true)
}

// const pathElts = splitPath(path);
// const buffer = Buffer.alloc(1 + pathElts.length * 4);

// buffer[0] = pathElts.length;

// pathElts.forEach((element, index) => {

//// from docs: buf.writeUInt32BE(value[, offset])
//   buffer.writeUInt32BE(element, 1 + 4 * index);

// });

// const verifyMsg = Buffer.from("via lumina", "ascii");
// apdus.push(Buffer.concat([buffer, verifyMsg]));
// let keepAlive = false;

// index 0
// element 2147483692
// thing after element in write:  1
// index 1
// element 2147483796
// thing after element in write:  5
// index 2
// element 2147483648
// thing after element in write:  9

// the buffer data (as a vec) should look like this:
// [3, 128,   0,   0, 44, 128, 0,   0, 148, 128,  0,   0, 0]
// but it looks like this:
// [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 44, 128, 0, 0, 148, 128, 0, 0, 0]

// there are just too many 0 spacers

// this fn should mimic the above JS
fn create_a_data_buffer_like_in_js(path: &str) -> Vec<u8> {
    let path_elts = split_path(path);

    let mut buffer = vec![0; 1 + path_elts.len() * 4];
    buffer[0] = path_elts.len() as u8;

    for (index, &element) in path_elts.iter().enumerate() {
        let mut temp_buffer = vec![0; 4];
        temp_buffer
            .write_u32::<BigEndian>(element)
            .expect("Failed to write element to buffer");
        println!("{temp_buffer:?}");

        buffer.write(&temp_buffer);
    }
    println!("BUFFER: {:?}", buffer);

    buffer
}

fn bip_from_index(index: u32) -> slip10::BIP32Path {
    let path = format!("m/44'/148'/{index}'");
    path.parse().unwrap() // this is basically the same thing as slip10::BIP32Path::from_str

    // the device handles this part: https://github.com/AhaLabs/rs-sep5/blob/9d6e3886b4b424dd7b730ec24c865f6fad5d770c/src/seed_phrase.rs#L86
}

/// Converts BIP32Path into bytes (`Vec<u8>`)
fn hd_path_to_bytes(hd_path: &slip10::BIP32Path) -> Vec<u8> {
    (0..hd_path.depth())
        .map(|index| {
            let value = *hd_path.index(index).unwrap();
            value.to_be_bytes()
        })
        .flatten()
        .collect::<Vec<u8>>()
}

pub fn get_public_key_with_display_flag(
    hd_path: slip10::BIP32Path,
    display_and_confirm: bool,
) -> Result<stellar_strkey::ed25519::PublicKey, LedgerError> {
    // instantiate the connection to Ledger
    // will return an error if Ledger is not connected
    let transport = get_transport()?;

    let hd_path_as_string = hd_path.to_string();
    let hd_path_bytes = create_a_data_buffer_like_in_js(&hd_path_as_string);

    let js_hd_path_bytes: Vec<u8> = [3, 128, 0, 0, 44, 128, 0, 0, 148, 128, 0, 0, 0].to_vec();

    let mut hd_path_to_bytes = hd_path_to_bytes(&hd_path);
    hd_path_to_bytes.insert(0, 3);
    println!("hd_path_to_bytes: {:?}", hd_path_to_bytes);

    // // hd_path must be converted into bytes to be sent as `data` to the Ledger
    // let hd_path_bytes = hd_path_to_bytes(&hd_path);
    // println!("hd_path_bytes: {:?}", hd_path_bytes);

    let command = APDUCommand {
        cla: CLA,
        ins: GET_PUBLIC_KEY,
        p1: 0x00, // Instruction parameter 1 (offset)
        p2: P2_GET_PUB_DISPLAY,
        data: js_hd_path_bytes,
    };
    tracing::info!("APDU in: {}", hex::encode(&command.serialize()));

    match transport.exchange(&command) {
        Ok(response) => {
            tracing::info!(
                "APDU out: {}\nAPDU ret code: {:x}",
                hex::encode(response.apdu_data()),
                response.retcode(),
            );
            // Ok means we successfully exchanged with the Ledger
            // but doesn't mean our request succeeded
            // we need to check it based on `response.retcode`

            if response.retcode() == RETURN_CODE_OK {
                return Ok(
                    stellar_strkey::ed25519::PublicKey::from_payload(&response.data()).unwrap(),
                );
            } else {
                let retcode = response.retcode();

                let error_string = format!("Ledger APDU retcode: 0x{:X}", retcode);
                return Err(LedgerError::APDUExchangeError(error_string));
            }
        }
        Err(err) => return Err(LedgerError::LedgerHIDError(err)),
    };
}

fn get_transport() -> Result<TransportNativeHID, LedgerError> {
    // instantiate the connection to Ledger
    // will return an error if Ledger is not connected
    let hidapi = HidApi::new().map_err(LedgerError::HidApiError)?;
    TransportNativeHID::new(&hidapi).map_err(LedgerError::LedgerHidError)
}
