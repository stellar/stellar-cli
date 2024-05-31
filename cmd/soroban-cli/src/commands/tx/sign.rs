use std::io;

use soroban_rpc::Client;
// use crossterm::{
//     event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
//     execute,
//     terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
// };
use soroban_sdk::xdr::{
    self, Limits, MuxedAccount, SequenceNumber, Transaction, TransactionEnvelope, Uint256, WriteXdr,
};
use stellar_strkey::Strkey;

use crate::signer::{self, native, Stellar};

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
    Ledger(#[from] stellar_ledger::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
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
        let envelope = self.sign().await?;
        println!("{}", envelope.to_xdr_base64(Limits::none())?.trim());
        Ok(())
    }

    pub async fn sign(&self) -> Result<TransactionEnvelope, Error> {
        let txn = super::xdr::unwrap_envelope_v1()?;
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

    pub fn network_passphrase(&self) -> Result<String, Error> {
        Ok(self.config.get_network()?.network_passphrase)
    }

    pub async fn sign_with_signer(
        &self,
        signer: &impl Stellar,
        mut txn: Transaction,
    ) -> Result<TransactionEnvelope, Error> {
        let key = signer.get_public_key().await.unwrap();
        let account = Strkey::PublicKeyEd25519(key);
        let client = Client::new(&self.config.get_network()?.rpc_url)?;
        txn.seq_num = SequenceNumber(client.get_account(&account.to_string()).await?.seq_num.0 + 1);
        txn.source_account = MuxedAccount::Ed25519(Uint256(key.0));
        eprintln!("Account {account}");
        Ok(signer
            .sign_txn(txn, &self.network_passphrase()?)
            .await
            .unwrap())
    }

    pub async fn sign_file(&self, mut txn: Transaction) -> Result<TransactionEnvelope, Error> {
        let key = self.config.key_pair()?;
        let address = key.get_public_key().await?;
        let client = Client::new(&self.config.get_network()?.rpc_url)?;
        txn.seq_num = SequenceNumber(client.get_account(&address.to_string()).await?.seq_num.0 + 1);
        self.prompt_user()?;
        Ok(key.sign_txn(txn, &self.network_passphrase()?).await?)
    }

    pub async fn sign_ledger(&self, txn: Transaction) -> Result<TransactionEnvelope, Error> {
        let index: u32 = self
            .config
            .hd_path
            .unwrap_or_default()
            .try_into()
            .expect("usize bigger than u32");
        let signer = native(index)?;
        self.sign_with_signer(&signer, txn).await
    }
}
