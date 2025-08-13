use stellar_xdr::curr::MuxedAccount;

use crate::{
    commands::{
        global,
        tx::xdr::{tx_envelope_from_input, Error as XdrParsingError},
    },
    config::{self, locator, network},
    xdr::{self, SequenceNumber, TransactionEnvelope, WriteXdr},
};

#[derive(clap::Parser, Debug, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,
    #[command(flatten)]
    pub locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrStdin(#[from] XdrParsingError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("V0 and fee bump transactions are not supported")]
    Unsupported,
    #[error(transparent)]
    RpcClient(#[from] crate::rpc::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] config::network::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let mut tx = tx_envelope_from_input(&None)?;
        self.update_tx_env(&mut tx, global_args).await?;
        println!("{}", tx.to_xdr_base64(xdr::Limits::none())?);
        Ok(())
    }

    pub async fn update_tx_env(
        &self,
        tx_env: &mut TransactionEnvelope,
        _global: &global::Args,
    ) -> Result<(), Error> {
        match tx_env {
            TransactionEnvelope::Tx(transaction_v1_envelope) => {
                let tx_source_acct = &transaction_v1_envelope.tx.source_account;
                let current_seq_num = self.current_seq_num(tx_source_acct).await?;
                let next_seq_num = current_seq_num + 1;
                transaction_v1_envelope.tx.seq_num = SequenceNumber(next_seq_num);
            }
            TransactionEnvelope::TxV0(_) | TransactionEnvelope::TxFeeBump(_) => {
                return Err(Error::Unsupported);
            }
        }
        Ok(())
    }

    async fn current_seq_num(&self, tx_source_acct: &MuxedAccount) -> Result<i64, Error> {
        let network = &self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;

        let address = tx_source_acct.to_string();

        let account = client.get_account(&address).await?;
        Ok(*account.seq_num.as_ref())
    }
}
