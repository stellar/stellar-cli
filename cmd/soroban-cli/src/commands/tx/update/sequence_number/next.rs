use crate::{
    commands::{
        global,
        tx::xdr::{tx_envelope_from_input, Error as XdrParsingError},
    },
    config,
    xdr::{self, SequenceNumber, TransactionEnvelope, WriteXdr},
};

#[derive(clap::Parser, Debug, Clone)]
pub struct Cmd {
    #[command(flatten)]
    pub config: config::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrStdin(#[from] XdrParsingError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("only V1 transactions are supported")]
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
                let current_seq_num = self.current_seq_num().await?;
                let next_seq_num = current_seq_num + 1;
                transaction_v1_envelope.tx.seq_num = SequenceNumber(next_seq_num);
            }
            TransactionEnvelope::TxV0(_) | TransactionEnvelope::TxFeeBump(_) => {
                return Err(Error::Unsupported);
            }
        };
        Ok(())
    }

    async fn current_seq_num(&self) -> Result<i64, Error> {
        let network = &self.config.get_network()?;
        let client = network.rpc_client()?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;

        let muxed_account = self.config.source_account().await?;

        let bytes = match muxed_account {
            soroban_sdk::xdr::MuxedAccount::Ed25519(uint256) => uint256.0,
            soroban_sdk::xdr::MuxedAccount::MuxedEd25519(muxed_account) => muxed_account.ed25519.0,
        };
        let address = stellar_strkey::ed25519::PublicKey(bytes).to_string();

        let account = client.get_account(&address).await?;
        Ok(*account.seq_num.as_ref())
    }
}
