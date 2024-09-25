use crate::{
    commands::{global, txn_result::TxnEnvelopeResult},
    config::{self, address, data, network, secret},
    fee,
    rpc::{self, Client, GetTransactionResponse},
    tx::builder::{self, TxExt},
    xdr::{self, Limits, WriteXdr},
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
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

impl Args {
    pub async fn tx(&self, body: xdr::OperationBody) -> Result<xdr::Transaction, Error> {
        let source_account = self.source_account()?;
        let seq_num = self
            .config
            .next_sequence_number(&source_account.to_string())
            .await?;
        let operation = xdr::Operation {
            source_account: self.with_source_account.map(Into::into),
            body,
        };
        Ok(xdr::Transaction::new_tx(
            source_account,
            self.fee.fee,
            seq_num,
            operation,
        ))
    }

    pub fn client(&self) -> Result<Client, Error> {
        let network = self.config.get_network()?;
        Ok(Client::new(&network.rpc_url)?)
    }

    pub async fn handle<T: builder::Operation>(
        &self,
        op: &T,
        global_args: &global::Args,
    ) -> Result<TxnEnvelopeResult<GetTransactionResponse>, Error> {
        let tx = self.tx(op.build_body()).await?;
        self.handle_tx(tx, global_args).await
    }
    pub async fn handle_and_print<T: builder::Operation>(
        &self,
        op: &T,
        global_args: &global::Args,
    ) -> Result<(), Error> {
        let res = self.handle(op, global_args).await?;
        if let TxnEnvelopeResult::TxnEnvelope(tx) = res {
            println!("{}", tx.to_xdr_base64(Limits::none())?);
        };
        Ok(())
    }

    pub async fn handle_tx(
        &self,
        tx: xdr::Transaction,
        args: &global::Args,
    ) -> Result<TxnEnvelopeResult<GetTransactionResponse>, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        if self.fee.build_only {
            return Ok(TxnEnvelopeResult::TxnEnvelope(tx.into()));
        }

        let txn_resp = client
            .send_transaction_polling(&self.config.sign_with_local_key(tx).await?)
            .await?;

        if !args.no_cache {
            data::write(txn_resp.clone().try_into().unwrap(), &network.rpc_uri()?)?;
        }

        Ok(TxnEnvelopeResult::Res(txn_resp))
    }

    pub fn source_account(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        Ok(self.config.source_account()?)
    }
}
