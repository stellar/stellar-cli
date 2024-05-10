use std::io;

// use crossterm::{
//     event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
//     execute,
//     terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
// };
use soroban_sdk::xdr::{self, Limits, TransactionEnvelope, WriteXdr};

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
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Confirm that a signature can be signed by the given keypair automatically.
    #[arg(long, short = 'y')]
    yes: bool,
    #[clap(flatten)]
    pub xdr_args: super::xdr::Args,
    #[clap(flatten)]
    pub config: super::super::config::Args,
}

impl Cmd {
    #[allow(clippy::unused_async)]
    pub async fn run(&self) -> Result<(), Error> {
        let envelope = self.sign()?;
        println!("{}", envelope.to_xdr_base64(Limits::none())?.trim());
        Ok(())
    }

    pub fn sign(&self) -> Result<TransactionEnvelope, Error> {
        let source = &self.config.source_account;
        tracing::debug!("signing transaction with source account {}", source);
        let txn = self.xdr_args.txn()?;
        let key = self.config.key_pair()?;
        let address =
            stellar_strkey::ed25519::PublicKey::from_payload(key.verifying_key().as_bytes())?;
        let in_memory = InMemory {
            network_passphrase: self.config.get_network()?.network_passphrase,
            keypairs: vec![key],
        };
        self.prompt_user()?;
        Ok(in_memory.sign_txn(txn, &stellar_strkey::Strkey::PublicKeyEd25519(address))?)
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
}