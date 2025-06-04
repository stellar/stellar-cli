use clap::{command, Subcommand};
use std::fmt::Debug;
use prettytable::{format::{self, FormatBuilder, LinePosition, LineSeparator}, Cell, Row, Table};

use crate::{
    commands::global,
    config::network,
    xdr::{
        Hash,
        TransactionMeta,
        SorobanTransactionMetaExt,
    },
};

mod args;
mod envelope;
mod meta;
mod result;

#[derive(Debug, clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Cmd {
    #[command(subcommand)]
    subcommand: Option<FetchCommands>,

    #[command(flatten)]
    default: DefaultArgs,
}

#[derive(Debug, Subcommand)]
pub enum FetchCommands {
    /// Fetch the transaction result
    Result(result::Cmd),
    /// Fetch the transaction meta
    Meta(meta::Cmd),
    /// Fetch the transaction envelope
    #[command(hide = true)]
    Envelope(envelope::Cmd),
}

#[derive(Debug, clap::Args)]
struct DefaultArgs {
    /// Hash of transaction to fetch
    #[arg(long)]
    pub hash: Option<Hash>,

    #[command(flatten)]
    pub network: Option<network::Args>,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: Option<args::OutputFormat>,

    #[arg(long)]
    pub fee_only: bool
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Result(#[from] result::Error),
    #[error(transparent)]
    Meta(#[from] meta::Error),
    #[error(transparent)]
    Envelope(#[from] envelope::Error),
    #[error("transaction meta extension type not supported: {ext_type}")]
    MetaExtNotSupported { ext_type: String },
    #[error("transaction meta version not supported: {version}")]
    MetaVersionNotSupported { version: String },
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        if self.default.fee_only {
            return self.fee_only(global_args).await
        }

        match &self.subcommand {
            Some(FetchCommands::Result(cmd)) => cmd.run(global_args).await?,
            Some(FetchCommands::Meta(cmd)) => cmd.run(global_args).await?,
            Some(FetchCommands::Envelope(cmd)) => cmd.run(global_args).await?,
            None => {
                envelope::Cmd {
                    args: args::Args {
                        hash: self
                            .default
                            .hash
                            .clone()
                            .expect("Transaction hash is required but was not provided."),
                        network: self.default.network.clone().unwrap(),
                        output: self.default.output.unwrap(),
                    },
                }
                .run(global_args)
                .await?;
            }
        }
        Ok(())
    }

    async fn fee_only(&self, global_args: &global::Args) -> Result<(), Error> {
        let args =  args::Args {
            hash: self.default.hash.clone().unwrap(),
            network: self.default.network.clone().unwrap(),
            output: self.default.output.unwrap(),
        };

        let resp = args.fetch_transaction(global_args).await.unwrap();
        let tx_result = resp.clone().result.unwrap();
        let tx_meta = resp.clone().result_meta.unwrap();
        let fee = tx_result.fee_charged;
        let (non_refundable_resource_fee, refundable_resource_fee) = match tx_meta.clone() {
           TransactionMeta::V0(_) => {
                return Err(Error::MetaVersionNotSupported { version: "TransactionMeta::V0".to_string() });
            },
            TransactionMeta::V1(_) => {
                return Err(Error::MetaVersionNotSupported { version: "TransactionMeta::V1".to_string() });
            },
            TransactionMeta::V2(_) => {
                return Err(Error::MetaVersionNotSupported { version: "TransactionMeta::V2".to_string() });
            },
            TransactionMeta::V3(meta) => {
                match meta.soroban_meta.unwrap().ext {
                    SorobanTransactionMetaExt::V0 => {
                        return Err(Error::MetaExtNotSupported { ext_type: "SorobanTransactionMetaExt::V0".to_string() })
                    },
                    SorobanTransactionMetaExt::V1(v1) => {
                        (v1.total_non_refundable_resource_fee_charged, v1.total_refundable_resource_fee_charged)
                    },
                }
            },
        };

        FeeTable{ fee, non_refundable_resource_fee, refundable_resource_fee }.print();

        Ok(())
    }

}

struct FeeTable {
    fee: i64,
    non_refundable_resource_fee: i64,
    refundable_resource_fee: i64,
}


impl FeeTable {
    fn inclusion_fee(&self) -> i64 {
        self.fee - self.resource_fee()
    }
    
    fn resource_fee(&self) -> i64 {
        self.non_refundable_resource_fee + self.refundable_resource_fee
    }

    fn print(&self) {
        let table_format = FormatBuilder::new()
                             .column_separator('│')
                             .borders('│')
                             .separators(&[LinePosition::Top],
                                         LineSeparator::new('─',
                                                            '─',
                                                            '┌',
                                                            '┐'))
                             .separators(&[LinePosition::Intern],
                                         LineSeparator::new('─',
                                                            '─',
                                                            '├',
                                                            '┤'))
                             .separators(&[LinePosition::Bottom],
                                         LineSeparator::new('─',
                                                            '─',
                                                            '└',
                                                            '┘'))
                             .padding(1, 1)
                             .build();

        let mut table = Table::new();

        // Optional: customize borders
        // table.set_format(*format::consts::FORMAT_BOX_CHARS);
        table.set_format(table_format);

        // First row: single wide cell (horizontally spans 2 columns)
        table.add_row(Row::new(vec![
            Cell::new(&format!("tx.fee: {}", self.fee))
                .style_spec("b")
                .with_hspan(3),
        ]));

        // Second row: two separate cells
        table.add_row(Row::new(vec![
            Cell::new(&format!("tx.v1.sorobanData.resourceFee: {}", self.resource_fee()))
                .style_spec("FY")
                .with_hspan(2),
            Cell::new(&format!("inclusion fee: {}", self.inclusion_fee())),
        ]));

        table.add_row(Row::new(vec![
            Cell::new(&format!("fixed resource fee: {}", self.non_refundable_resource_fee))
                .style_spec("FY"),
            Cell::new(&format!("refundable resource fee: {}", self.refundable_resource_fee))
                .style_spec("FY"),
            Cell::new(&format!("inclusion fee: {}", self.inclusion_fee())),
        ]));

        table.printstd();
    }
}