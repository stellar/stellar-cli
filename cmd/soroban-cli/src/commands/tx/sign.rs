use std::io;

use crate::xdr::{self, Limits, Transaction, TransactionEnvelope, WriteXdr};

use crate::signer::{self, native, LocalKey};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Signer(#[from] signer::Error),
    #[error(transparent)]
    Config(#[from] super::super::config::Error),
    #[error(transparent)]
    StellarStrkey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Ledger(#[from] stellar_ledger::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error("only transaction v1 is supported")]
    TransactionV1Expected,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Confirm that a signature can be signed by the given keypair automatically.
    #[arg(long, short = 'y')]
    pub yes: bool,
    #[clap(flatten)]
    pub config: super::super::config::Args,
    /// How to sign transaction
    #[arg(long, value_enum, default_value = "file")]
    pub signer: SignerType,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum SignerType {
    File,
    Ledger,
}

impl Cmd {
    #[allow(clippy::unused_async)]
    pub async fn run(&self) -> Result<(), Error> {
        let txn = super::xdr::unwrap_envelope_v1_from_stdin()?;
        let envelope = self.sign(txn).await?;
        println!("{}", envelope.to_xdr_base64(Limits::none())?.trim());
        Ok(())
    }

    pub async fn sign(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        match self.signer {
            SignerType::File => Ok(self
                .config
                .sign(&LocalKey::new(self.config.key_pair()?, !self.yes), tx)
                .await?),
            SignerType::Ledger => self.sign_ledger(tx).await,
        }
    }

    pub async fn sign_ledger(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        let index: u32 = self
            .config
            .hd_path
            .unwrap_or_default()
            .try_into()
            .expect("usize bigger than u32");
        let signer = native(index)?;
        Ok(self.config.sign(&signer, tx).await?)
    }
}
