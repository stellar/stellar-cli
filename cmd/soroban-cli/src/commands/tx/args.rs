use soroban_rpc::GetTransactionResponse;

use crate::{
    commands::{global, txn_result::TxnResult},
    config::{self, data, network, secret},
    fee,
    rpc::{self, Client},
    tx::builder,
    xdr,
};

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    #[clap(flatten)]
    pub fee: fee::Args,
    #[clap(flatten)]
    pub config: config::Args,
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
    Strkey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Tx(#[from] builder::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
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

        // let txn = client.simulate_and_assemble_transaction(&tx).await?;
        // let txn = self.fee.apply_to_assembled_txn(txn).transaction().clone();

        // if self.fee.sim_only {
        //     return Ok(TxnResult::Txn(txn));
        // }

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
