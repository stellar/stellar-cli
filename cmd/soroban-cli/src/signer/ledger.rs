pub use ledger_impl::*;
use crate::xdr::{
    DecoratedSignature, Transaction,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Ledger Device keys are not allowed: additional-libs feature must be enabled")]
    FeatureNotEnabled,
}

#[cfg(not(feature = "additional-libs"))]
mod ledger_impl {
    use super::*;
    use std::marker::PhantomData;

    pub trait Exchange {}
    pub struct Ledger<T: Exchange> {
        _marker: PhantomData<T>,
    }

    impl <T: Exchange> Ledger<T> {
        pub async fn sign_transaction_hash(
            &self,
            tx_hash: &[u8; 32],
        ) -> Result<DecoratedSignature, Error> {
            Err(Error::FeatureNotEnabled)
        }

        pub async fn sign_transaction(
            &self,
            tx: Transaction,
            network_passphrase: &str,
        ) -> Result<DecoratedSignature, Error> {
            Err(Error::FeatureNotEnabled)
        }

        pub async fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
            Err(Error::FeatureNotEnabled)
        }
    }

    pub struct GenericExchange {}

    impl Exchange for GenericExchange {
    }

    impl GenericExchange {
    }

    pub async fn new(_hd_path: u32) -> Result<Ledger<GenericExchange>, Error> {
        Err(Error::FeatureNotEnabled)
    }
}

#[cfg(feature = "additional-libs")]
mod ledger_impl {
    #[cfg(all(feature = "additional-libs", not(feature = "emulator-tests")))]
    pub async fn new(hd_path: u32) -> Result<Ledger<stellar_ledger::TransportNativeHID>, Error> {
        let signer = stellar_ledger::native()?;
        Ok(Ledger {
            index: hd_path,
            signer,
        })
    }

    #[cfg(all(feature = "additional-libs", feature = "emulator-tests"))]
    pub async fn new(
        hd_path: u32,
    ) -> Result<Ledger<stellar_ledger::emulator_test_support::http_transport::Emulator>, Error> {
        use stellar_ledger::emulator_test_support::ledger as emulator_ledger;
        // port from SPECULOS_PORT ENV var
        let host_port: u16 = std::env::var("SPECULOS_PORT")
            .expect("SPECULOS_PORT env var not set")
            .parse()
            .expect("port must be a number");
        let signer = emulator_ledger(host_port).await;

        Ok(Ledger {
            index: hd_path,
            signer,
        })
    }

}