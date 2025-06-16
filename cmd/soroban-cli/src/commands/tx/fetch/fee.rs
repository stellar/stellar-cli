use super::args;
use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash, SorobanTransactionMetaExt, TransactionMeta},
};
use clap::{command, Parser};
use prettytable::{
    format::{FormatBuilder, LinePosition, LineSeparator},
    Cell, Row, Table,
};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    args: args::Args,

    /// Output format for fee command
    #[arg(long, default_value = "table")]
    pub output: FeeOutputFormat,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum FeeOutputFormat {
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
    /// Formatted in a table comparing fee types
    #[default]
    Table,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Args(#[from] args::Error),
    #[error("{message}")]
    NotSupported { message: String },
    #[error("transaction {tx_hash} not found on {network} network")]
    NotFound { tx_hash: Hash, network: String },
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
}

const DEFAULT_FEE_VALUE: i64 = 0;

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let resp = self.args.fetch_transaction(global_args).await?;
        let tx_result = resp.result.clone().unwrap();
        let tx_meta = resp.result_meta.clone().unwrap();

        let fee = tx_result.fee_charged;
        let (non_refundable_resource_fee, refundable_resource_fee) = match tx_meta.clone() {
            TransactionMeta::V0(_) | TransactionMeta::V1(_) | TransactionMeta::V2(_) => {
                (DEFAULT_FEE_VALUE, DEFAULT_FEE_VALUE)
            }
            TransactionMeta::V3(meta) => {
                if let Some(soroban_meta) = meta.soroban_meta {
                    match soroban_meta.ext {
                        SorobanTransactionMetaExt::V0 => (DEFAULT_FEE_VALUE, DEFAULT_FEE_VALUE),
                        SorobanTransactionMetaExt::V1(v1) => (
                            v1.total_non_refundable_resource_fee_charged,
                            v1.total_refundable_resource_fee_charged,
                        ),
                    }
                } else {
                    (DEFAULT_FEE_VALUE, DEFAULT_FEE_VALUE)
                }
            }
        };

        let fee_table = FeeTable::new(fee, non_refundable_resource_fee, refundable_resource_fee);

        match self.output {
            FeeOutputFormat::Json => {
                println!("{}", serde_json::to_string(&fee_table)?);
            }
            FeeOutputFormat::JsonFormatted => {
                println!("{}", serde_json::to_string_pretty(&fee_table)?);
            }
            FeeOutputFormat::Table => {
                fee_table.print();
            }
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FeeTable {
    pub fee: i64,
    pub resource_fee: i64,
    pub inclusion_fee: i64,
    pub non_refundable_resource_fee: i64,
    pub refundable_resource_fee: i64,
}

impl FeeTable {
    fn new(fee: i64, non_refundable_resource_fee: i64, refundable_resource_fee: i64) -> Self {
        let resource_fee = non_refundable_resource_fee + refundable_resource_fee;
        FeeTable {
            fee,
            resource_fee,
            inclusion_fee: fee - resource_fee,
            non_refundable_resource_fee,
            refundable_resource_fee,
        }
    }

    fn print(&self) {
        let table_format = FormatBuilder::new()
            .column_separator('│')
            .borders('│')
            .separators(&[LinePosition::Top], LineSeparator::new('─', '─', '┌', '┐'))
            .separators(
                &[LinePosition::Intern],
                LineSeparator::new('─', '─', '├', '┤'),
            )
            .separators(
                &[LinePosition::Bottom],
                LineSeparator::new('─', '─', '└', '┘'),
            )
            .padding(1, 1)
            .build();

        let mut table = Table::new();

        table.set_format(table_format);

        table.add_row(Row::new(vec![Cell::new(&format!("tx.fee: {}", self.fee))
            .style_spec("b")
            .with_hspan(3)]));

        table.add_row(Row::new(vec![
            Cell::new(&format!(
                "tx.v1.sorobanData.resourceFee: {}",
                self.resource_fee
            ))
            .style_spec("FY")
            .with_hspan(2),
            Cell::new(&format!("inclusion fee: {}", self.inclusion_fee)),
        ]));

        table.add_row(Row::new(vec![
            Cell::new(&format!(
                "non-refundable resource fee: {}\n\ncalculated based on tx.v1.sorobanData.resources.*\n\ninstructions\nread\nwrite\nbandwidth (size of tx)",
                self.non_refundable_resource_fee
            ))
            .style_spec("FY"),
            Cell::new(&format!(
                "refundable resource fee: {}\n\n\n\nrent\nevents\nreturn value",
                self.refundable_resource_fee
            ))
            .style_spec("FY"),
            Cell::new(&format!("inclusion fee: {}", self.inclusion_fee)),
        ]));

        table.printstd();
    }
}
