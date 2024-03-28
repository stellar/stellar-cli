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

enum Error {}

#[cfg(test)]
mod test {
    use crate::{app::new_get_transport, emulator::Emulator};

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

    #[tokio::test]
    #[serial]
    async fn test_get_public_key() {
        let transport = new_get_transport().unwrap();
        let ledger = app::Ledger::new(transport);
        let public_key = ledger.get_public_key(0).await;
        println!("{public_key:?}");
        assert!(public_key.is_ok());
    }

    // #[test]
    #[tokio::test]
    async fn test_my_emulator() {
        let mut e = Emulator::new().await;
        let start_result = e.run().await;
        assert!(start_result.is_ok());

        let stop_result = e.stop().await;
        assert!(stop_result.is_ok());
    }
}
