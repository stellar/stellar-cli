use crate::xdr::{self, OperationBody, Transaction, TransactionEnvelope};

pub use ledger_impl::*;

// Operations the Ledger Stellar app cannot pretty-print. When any of these
// appears in the envelope, the device falls into hash-signing mode (requires
// `Hash Signing` enabled in app settings); sending `SIGN_TX` (0x04) for them
// ends up at the same UX as `SIGN_TX_HASH` (0x08) but with extra device-side
// parsing churn, so the CLI sends the hash directly.
pub fn is_soroban_tx(tx: &Transaction) -> bool {
    tx.operations.iter().any(|op| {
        matches!(
            op.body,
            OperationBody::InvokeHostFunction(_)
                | OperationBody::ExtendFootprintTtl(_)
                | OperationBody::RestoreFootprint(_),
        )
    })
}

pub fn is_soroban_tx_env(tx_env: &TransactionEnvelope) -> bool {
    match tx_env {
        TransactionEnvelope::Tx(v1) => is_soroban_tx(&v1.tx),
        TransactionEnvelope::TxFeeBump(fb) => {
            let xdr::FeeBumpTransactionInnerTx::Tx(inner) = &fb.tx.inner_tx;
            is_soroban_tx(&inner.tx)
        }
        TransactionEnvelope::TxV0(_) => false,
    }
}

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

    #[error("Transaction envelope type not supported for Ledger signing")]
    UnsupportedTransactionEnvelopeType,
}

#[cfg(feature = "additional-libs")]
mod ledger_impl {
    use super::{is_soroban_tx_env, Error};
    use crate::{
        print::Print,
        utils::transaction_env_hash,
        xdr::{
            DecoratedSignature, Hash, Signature, SignatureHint, TransactionEnvelope,
            TransactionV1Envelope,
        },
    };
    use ed25519_dalek::Signature as Ed25519Signature;
    use sha2::{Digest, Sha256};
    use stellar_ledger::{Blob as _, Exchange, LedgerSigner};
    use stellar_xdr::curr::FeeBumpTransactionEnvelope;

    #[cfg(not(feature = "emulator-tests"))]
    pub type LedgerType = Ledger<stellar_ledger::TransportNativeHID>;
    #[cfg(feature = "emulator-tests")]
    pub type LedgerType = Ledger<stellar_ledger::emulator_test_support::http_transport::Emulator>;

    // Pure-data signer for Ledger identities. Mirrors `SecureStoreEntry`:
    // holds no live transport, opens HID lazily inside each sign call so the
    // device stays free between operations and can never collide with a
    // concurrent transport elsewhere in the process.
    pub struct LedgerEntry {
        pub hd_path: u32,
        pub public_key: Option<stellar_strkey::ed25519::PublicKey>,
    }

    impl LedgerEntry {
        // Sign a transaction envelope on the Ledger device.
        //
        // Classic envelopes are clear-signed (APDU SIGN_TX, 0x04): the full
        // `TransactionSignaturePayload` is sent so the device parses and
        // displays each operation for verification.
        //
        // Soroban envelopes (envelopes containing `InvokeHostFunction`,
        // `ExtendFootprintTtl`, or `RestoreFootprint`) are blind-signed (APDU
        // SIGN_TX_HASH, 0x08): the Ledger Stellar app cannot pretty-print
        // those operations, so the device shows the transaction hash and
        // requires `Hash Signing` enabled in app settings.
        pub async fn sign_tx_env(
            &self,
            tx_env: &TransactionEnvelope,
            network_passphrase: &str,
            print: &Print,
        ) -> Result<DecoratedSignature, Error> {
            let live = new(self.hd_path).await?;
            let key = match self.public_key {
                Some(pk) => pk,
                None => live.public_key().await?,
            };
            let hint = SignatureHint(key.0[28..].try_into()?);

            let signature_bytes = if is_soroban_tx_env(tx_env) {
                let tx_hash = transaction_env_hash(tx_env, network_passphrase)?;
                print.infoln(format!(
                    "Approve the transaction {} on your Ledger device…",
                    hex::encode(tx_hash),
                ));
                live.signer
                    .sign_transaction_hash(live.index, &tx_hash)
                    .await?
            } else {
                print.infoln("Approve the transaction on your Ledger device…");
                let network_id = Hash(Sha256::digest(network_passphrase).into());
                match tx_env {
                    TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) => {
                        live.signer
                            .sign_transaction(live.index, tx.clone(), network_id)
                            .await?
                    }
                    TransactionEnvelope::TxFeeBump(FeeBumpTransactionEnvelope { tx, .. }) => {
                        live.signer
                            .sign_fee_bump_transaction(live.index, tx.clone(), network_id)
                            .await?
                    }
                    TransactionEnvelope::TxV0(_) => {
                        return Err(Error::UnsupportedTransactionEnvelopeType);
                    }
                }
            };

            Ok(DecoratedSignature {
                hint,
                signature: Signature(signature_bytes.try_into()?),
            })
        }

        // Blind-sign a 32-byte payload. Used for Soroban authorization-entry
        // preimage digests, which have no on-device pretty-print.
        pub async fn sign_payload(&self, payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
            let live = new(self.hd_path).await?;
            let bytes = live
                .signer
                .sign_transaction_hash(live.index, &payload)
                .await?;
            Ok(Ed25519Signature::from_bytes(bytes.as_slice().try_into()?))
        }
    }

    pub struct Ledger<T: Exchange> {
        pub(crate) index: u32,
        pub(crate) signer: LedgerSigner<T>,
    }

    #[cfg(not(feature = "emulator-tests"))]
    #[allow(clippy::unused_async)]
    pub async fn new(hd_path: u32) -> Result<Ledger<stellar_ledger::TransportNativeHID>, Error> {
        let signer = stellar_ledger::native()?;
        Ok(Ledger {
            index: hd_path,
            signer,
        })
    }

    #[cfg(feature = "emulator-tests")]
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
        pub async fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
            Ok(self.signer.get_public_key(&self.index.into()).await?)
        }
    }
}

#[cfg(not(feature = "additional-libs"))]
mod ledger_impl {
    use super::Error;
    use crate::{
        print::Print,
        xdr::{DecoratedSignature, TransactionEnvelope},
    };
    use ed25519_dalek::Signature as Ed25519Signature;
    use std::marker::PhantomData;

    pub type LedgerType = Ledger<GenericExchange>;

    pub trait Exchange {}
    pub struct Ledger<T: Exchange> {
        _marker: PhantomData<T>,
    }

    pub struct LedgerEntry {
        pub hd_path: u32,
        pub public_key: Option<stellar_strkey::ed25519::PublicKey>,
    }

    impl LedgerEntry {
        #[allow(clippy::unused_async)]
        pub async fn sign_tx_env(
            &self,
            _tx_env: &TransactionEnvelope,
            _network_passphrase: &str,
            _print: &Print,
        ) -> Result<DecoratedSignature, Error> {
            Err(Error::FeatureNotEnabled)
        }

        #[allow(clippy::unused_async)]
        pub async fn sign_payload(&self, _payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
            Err(Error::FeatureNotEnabled)
        }
    }

    #[allow(clippy::unused_async)]
    pub async fn new(_hd_path: u32) -> Result<Ledger<GenericExchange>, Error> {
        Err(Error::FeatureNotEnabled)
    }

    impl<T: Exchange> Ledger<T> {
        #[allow(clippy::unused_async)]
        pub async fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
            Err(Error::FeatureNotEnabled)
        }
    }

    pub struct GenericExchange {}

    impl Exchange for GenericExchange {}

    impl GenericExchange {}
}
