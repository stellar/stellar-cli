use clap::{command, Parser};

use crate::{commands::tx, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    #[clap(flatten)]
    pub op: Args,
}

#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    /// Line to change, either 4 or 12 alphanumeric characters, or "native" if not specified
    #[arg(long)]
    pub data_name: xdr::StringM<64>,
    /// Up to 64 bytes long hex string
    /// If not present then the existing Name will be deleted.
    /// If present then this value will be set in the `DataEntry`.
    #[arg(long)]
    pub data_value: Option<xdr::BytesM<64>>,
}

impl From<&Args> for xdr::OperationBody {
    fn from(cmd: &Args) -> Self {
        let data_value = cmd.data_value.clone().map(Into::into);
        let data_name = cmd.data_name.clone().into();
        xdr::OperationBody::ManageData(xdr::ManageDataOp {
            data_name,
            data_value,
        })
    }
}
