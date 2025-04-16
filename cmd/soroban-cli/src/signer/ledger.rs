use crate::xdr::{self, DecoratedSignature, Transaction};

pub use ledger_impl::*;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Ledger Device keys are not allowed: additional-libs feature must be enabled")]
    FeatureNotEnabled,

    #[cfg(feature = "additional-libs")]
    #[error(transparent)]
    StellarLedger(#[from] stellar_ledger::Error),

    #[error(transparent)]
    TryFromSlice(#[from] std::array::TryFromSliceError),

    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

#[cfg(all(feature = "additional-libs", not(feature = "emulator-tests")))]
pub type LedgerType = Ledger<stellar_ledger::TransportNativeHID>;
#[cfg(all(feature = "emulator-tests", feature = "additional-libs"))]
pub type LedgerType = Ledger<stellar_ledger::emulator_test_support::http_transport::Emulator>;
#[cfg(not(feature = "additional-libs"))]
pub type LedgerType = Ledger<GenericExchange>;

#[cfg(feature = "additional-libs")]
mod ledger_impl {
    use super::*;
    use crate::xdr::{Hash, Signature, SignatureHint};
    use sha2::{Digest, Sha256};
    use stellar_ledger::{Blob as _, Exchange, LedgerSigner};

    pub struct Ledger<T: Exchange> {
        pub(crate) index: u32,
        pub(crate) signer: LedgerSigner<T>,
    }

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
    ) -> Result<Ledger<stellar_ledger::emulator_test_support::http_transport::Emulator>, Error>
    {
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

    impl<T: Exchange> Ledger<T> {
        pub async fn sign_transaction_hash(
            &self,
            tx_hash: &[u8; 32],
        ) -> Result<DecoratedSignature, Error> {
            let key = self.public_key().await?;
            let hint = SignatureHint(key.0[28..].try_into()?);
            let signature = Signature(
                self.signer
                    .sign_transaction_hash(self.index, tx_hash)
                    .await?
                    .try_into()?,
            );
            Ok(DecoratedSignature { hint, signature })
        }

        pub async fn sign_transaction(
            &self,
            tx: Transaction,
            network_passphrase: &str,
        ) -> Result<DecoratedSignature, Error> {
            let network_id = Hash(Sha256::digest(network_passphrase).into());
            let signature = self
                .signer
                .sign_transaction(self.index, tx, network_id)
                .await?;
            let key = self.public_key().await?;
            let hint = SignatureHint(key.0[28..].try_into()?);
            let signature = Signature(signature.try_into()?);
            Ok(DecoratedSignature { hint, signature })
        }

        pub async fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
            Ok(self.signer.get_public_key(&self.index.into()).await?)
        }
    }
}

#[cfg(not(feature = "additional-libs"))]
mod ledger_impl {
    use super::*;
    use std::marker::PhantomData;

    pub trait Exchange {}
    pub struct Ledger<T: Exchange> {
        _marker: PhantomData<T>,
    }

    pub async fn new(_hd_path: u32) -> Result<Ledger<GenericExchange>, Error> {
        Err(Error::FeatureNotEnabled)
    }

    impl<T: Exchange> Ledger<T> {
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

    impl Exchange for GenericExchange {}

    impl GenericExchange {}
}
