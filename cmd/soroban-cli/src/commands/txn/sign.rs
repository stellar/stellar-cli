use std::io;

// use crossterm::{
//     event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
//     execute,
//     terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
// };
use soroban_sdk::xdr::{
    self, Limits, MuxedAccount, Transaction, TransactionEnvelope, Uint256, WriteXdr,
};
use stellar_ledger::{LedgerError, NativeSigner};
use stellar_strkey::Strkey;

use crate::signer::{self, InMemory, Stellar};

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
    #[error("User cancelled signing, perhaps need to add -y")]
    UserCancelledSigning,
    #[error(transparent)]
    Ledger(#[from] LedgerError),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Confirm that a signature can be signed by the given keypair automatically.
    #[arg(long, short = 'y', short = 'Y')]
    yes: bool,
    #[clap(flatten)]
    pub xdr_args: super::xdr::Args,
    #[clap(flatten)]
    pub config: super::super::config::Args,

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
        let envelope = self.sign().await?;
        println!("{}", envelope.to_xdr_base64(Limits::none())?.trim());
        Ok(())
    }

    pub async fn sign(&self) -> Result<TransactionEnvelope, Error> {
        let source = &self.config.source_account;
        tracing::debug!("signing transaction with source account {}", source);
        let txn = self.xdr_args.txn()?;
        match self.signer {
            SignerType::File => self.sign_file(txn).await,
            SignerType::Ledger => self.sign_ledger(txn).await,
        }
    }

    pub fn prompt_user(&self) -> Result<(), Error> {
        if self.yes {
            return Ok(());
        }
        Err(Error::UserCancelledSigning)
        // TODO use crossterm to prompt user for confirmation
        // // Set up the terminal
        // let mut stdout = io::stdout();
        // execute!(stdout, EnterAlternateScreen)?;
        // terminal::enable_raw_mode()?;

        // println!("Press 'y' or 'Y' for yes, any other key for no:");

        // loop {
        //     if let Event::Key(KeyEvent {
        //         code,
        //         modifiers: KeyModifiers::NONE,
        //         ..
        //     }) = event::read()?
        //     {
        //         match code {
        //             KeyCode::Char('y' | 'Y') => break,
        //             _ => return Err(Error::UserCancelledSigning),
        //         }
        //     }
        // }

        // // Clean up the terminal
        // terminal::disable_raw_mode()?;
        // execute!(stdout, LeaveAlternateScreen)?;
        // Ok(())
    }

    pub async fn sign_file(&self, txn: Transaction) -> Result<TransactionEnvelope, Error> {
        let key = self.config.key_pair()?;
        let address =
            stellar_strkey::ed25519::PublicKey::from_payload(key.verifying_key().as_bytes())?;
        let in_memory = InMemory {
            network_passphrase: self.config.get_network()?.network_passphrase,
            keypairs: vec![key],
        };
        self.prompt_user()?;
        Ok(in_memory
            .sign_txn(txn, &Strkey::PublicKeyEd25519(address))
            .await?)
    }

    pub async fn sign_ledger(&self, mut txn: Transaction) -> Result<TransactionEnvelope, Error> {
        let index: u32 = self
            .config
            .hd_path
            .unwrap_or_default()
            .try_into()
            .expect("usize bigger than u32");
        let signer: NativeSigner =
            (self.config.get_network()?.network_passphrase, index).try_into()?;
        let key = signer.as_ref().get_public_key(index).await.unwrap();
        let account = Strkey::PublicKeyEd25519(key);
        txn.source_account = MuxedAccount::Ed25519(Uint256(key.0));
        let bx_signer = Box::new(signer);
        Ok(bx_signer.sign_txn(txn, &account).await.unwrap())
    }
}
