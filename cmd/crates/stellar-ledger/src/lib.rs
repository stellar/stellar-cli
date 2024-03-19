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
use app::get_public_key;

enum Error {}

#[cfg(test)]
mod test {
    use super::*;
    use hidapi::HidApi;
    use ledger_transport_hid::TransportNativeHID;
    use log::info;
    use once_cell::sync::Lazy;
    use serial_test::serial;

    fn init_logging() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    fn hidapi() -> &'static HidApi {
        static HIDAPI: Lazy<HidApi> = Lazy::new(|| HidApi::new().expect("unable to get HIDAPI"));

        &HIDAPI
    }

    #[test]
    #[serial]
    fn list_all_devices() {
        init_logging();
        let api = hidapi();

        for device_info in api.device_list() {
            println!(
                "{:#?} - {:#x}/{:#x}/{:#x}/{:#x} {:#} {:#}",
                device_info.path(),
                device_info.vendor_id(),
                device_info.product_id(),
                device_info.usage_page(),
                device_info.interface_number(),
                device_info.manufacturer_string().unwrap_or_default(),
                device_info.product_string().unwrap_or_default()
            );
        }
    }

    #[test]
    #[serial]
    fn ledger_device_path() {
        init_logging();
        let api = hidapi();

        let mut ledgers = TransportNativeHID::list_ledgers(&api);

        let a_ledger = ledgers.next().expect("could not find any ledger device");
        println!("{:?}", a_ledger.path());
    }

    #[test]
    #[serial]
    fn test_get_public_key() {
        let public_key = get_public_key(0);
        println!("{public_key:?}");
    }
}
