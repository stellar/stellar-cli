use crate::commands::contract::arg_parsing::invoke_host_function_op_from_input;
use crate::{commands::tx, xdr};
use clap::Parser;
use std::ffi::OsString;
use stellar_xdr::curr::OperationBody;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    #[clap(flatten)]
    pub op: Args,
}

#[derive(Debug, clap::Args, Clone)]
#[allow(clippy::struct_excessive_bools, clippy::doc_markdown)]
pub struct Args {
    /// Base-64 InvokeContractArgs envelope XDR or file containing XDR to decode.
    #[arg(long)]
    pub xdr: OsString,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        let parameters = invoke_host_function_op_from_input(&cmd.op.xdr)?;

        Ok(OperationBody::InvokeHostFunction(parameters))
    }
}
