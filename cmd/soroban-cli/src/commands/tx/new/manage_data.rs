use clap::{command, Parser};

use crate::{
    commands::{global, tx},
    tx::builder,
    xdr,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    /// Line to change, either 4 or 12 alphanumeric characters, or "native" if not specified
    #[arg(long)]
    pub data_name: xdr::StringM<64>,
    /// Up to 64 bytes long hex string
    /// If not present then the existing Name will be deleted.
    /// If present then this value will be set in the `DataEntry`.
    #[arg(long)]
    pub data_value: Option<xdr::BytesM<64>>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tx(#[from] tx::args::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        self.tx.handle_and_print(self, global_args).await?;
        Ok(())
    }
}
impl builder::Operation for Cmd {
    fn build_body(&self) -> xdr::OperationBody {
        let data_value = self.data_value.clone().map(Into::into);
        xdr::OperationBody::ManageData(xdr::ManageDataOp {
            data_name: self.data_name.clone().into(),
            data_value,
        })
    }
}
