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
use soroban_rpc::GetTransactionResponse;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub(crate) args: FeeArgs,
}

#[derive(Debug, Clone, clap::Args)]
pub struct FeeArgs {
    /// Transaction hash to fetch
    #[arg(long)]
    pub hash: Hash,

    #[command(flatten)]
    pub network: network::Args,

    /// Output format for fee command
    #[arg(long, default_value = "table")]
    pub output: FeeOutputFormat,
}

impl FeeArgs {
    pub async fn fetch_transaction(
        &self,
        global_args: &global::Args,
    ) -> Result<GetTransactionResponse, Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let tx_hash = self.hash.clone();
        let tx = client.get_transaction(&tx_hash).await?;
        match tx.status.clone() {
            val if val == *"NOT_FOUND" => {
                if let Some(n) = &self.network.network {
                    return Err(Error::NotFound {
                        tx_hash,
                        network: n.to_string(),
                    });
                }
            }
            _ => {}
        }
        Ok(tx)
    }
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

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let resp = self.args.fetch_transaction(global_args).await?;
        let tx_result = resp.clone().result.unwrap();
        let tx_meta = resp.clone().result_meta.unwrap();
        let fee = tx_result.fee_charged;
        let (non_refundable_resource_fee, refundable_resource_fee) = match tx_meta.clone() {
            TransactionMeta::V0(_) => {
                return Err(Error::NotSupported {
                    message: "TransactionMeta::V0 not supported".to_string(),
                });
            }
            TransactionMeta::V1(_) => {
                return Err(Error::NotSupported {
                    message: "TransactionMeta::V1 not supported".to_string(),
                });
            }
            TransactionMeta::V2(_) => {
                return Err(Error::NotSupported {
                    message: "TransactionMeta::V2 not supported".to_string(),
                });
            }
            TransactionMeta::V3(meta) => {
                if let Some(soroban_meta) = meta.soroban_meta {
                    match soroban_meta.ext {
                        SorobanTransactionMetaExt::V0 => {
                            return Err(Error::NotSupported {
                                message: "SorobanTransactionMetaExt::V0 not supported".to_string(),
                            })
                        }
                        SorobanTransactionMetaExt::V1(v1) => (
                            v1.total_non_refundable_resource_fee_charged,
                            v1.total_refundable_resource_fee_charged,
                        ),
                    }
                } else {
                    return Err(Error::NotSupported {
                        message: "cannot get fee when soroban_meta is None".to_string(),
                    });
                }
            }
        };

        let fee_table = FeeTable::new(fee, non_refundable_resource_fee, refundable_resource_fee);

        match self.args.output {
            FeeOutputFormat::Json => {
                println!("{}", serde_json::to_string(&fee_table)?);
            }
            FeeOutputFormat::JsonFormatted => {
                println!("{}", serde_json::to_string_pretty(&fee_table)?);
            }
            FeeOutputFormat::Table => fee_table.print(),
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

        // Optional: customize borders
        // table.set_format(*format::consts::FORMAT_BOX_CHARS);
        table.set_format(table_format);

        // First row: single wide cell (horizontally spans 2 columns)
        table.add_row(Row::new(vec![Cell::new(&format!("tx.fee: {}", self.fee))
            .style_spec("b")
            .with_hspan(3)]));

        // Second row: two separate cells
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
                "fixed resource fee: {}",
                self.non_refundable_resource_fee
            ))
            .style_spec("FY"),
            Cell::new(&format!(
                "refundable resource fee: {}",
                self.refundable_resource_fee
            ))
            .style_spec("FY"),
            Cell::new(&format!("inclusion fee: {}", self.inclusion_fee)),
        ]));

        table.printstd();
    }
}
