use soroban_rpc::GetTransactionResponse;

use crate::{
    commands::{global, txn_result::TxnResult},
    config::{self, address, data, network, secret},
    fee,
    rpc::{self, Client},
    tx::builder,
};

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    #[clap(flatten)]
    pub fee: fee::Args,
    #[clap(flatten)]
    pub config: config::Args,
    //// The source account for the operation, Public key or Muxxed Account
    /// e.g. `GA3D5...` or `MA3D5...`
    #[arg(
        long,
        visible_alias = "with_source",
        env = "STELLAR_WITH_SOURCE_ACCOUNT"
    )]
    pub with_source_account: Option<address::Address>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Tx(#[from] builder::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
}

impl Args {
    pub async fn tx_builder(&self) -> Result<builder::Transaction, Error> {
        let source_account = self.source_account()?;
        let seq_num = self
            .config
            .next_sequence_number(&source_account.to_string())
            .await?;
        Ok(builder::Transaction::new(
            source_account,
            self.fee.fee,
            seq_num,
        ))
    }

    pub fn client(&self) -> Result<Client, Error> {
        let network = self.config.get_network()?;
        Ok(Client::new(&network.rpc_url)?)
    }

    pub async fn handle_tx(
        &self,
        tx: builder::Transaction,
        args: &global::Args,
    ) -> Result<TxnResult<GetTransactionResponse>, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        let tx = tx.build()?;
        if self.fee.build_only {
            return Ok(TxnResult::Txn(tx));
        }

        let txn_resp = client
            .send_transaction_polling(&self.config.sign_with_local_key(tx).await?)
            .await?;

        if !args.no_cache {
            data::write(txn_resp.clone().try_into().unwrap(), &network.rpc_uri()?)?;
        }

        Ok(TxnResult::Res(txn_resp))
    }

    pub fn source_account(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        Ok(self
            .config
            .account(&self.config.source_account)?
            .public_key(self.config.hd_path)?)
    }
}
