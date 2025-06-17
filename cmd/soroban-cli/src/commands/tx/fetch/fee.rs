use super::args;
use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash, SorobanTransactionMetaExt, TransactionEnvelope, TransactionMeta},
};
use clap::{command, Parser};
use prettytable::{
    format::{FormatBuilder, LinePosition, LineSeparator, TableFormat},
    Cell, Row, Table,
};
use serde::{Deserialize, Serialize};
use soroban_rpc::GetTransactionResponse;

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
    #[error("{field} is None, expected it to be Some")]
    None { field: String },
}

const DEFAULT_FEE_VALUE: i64 = 0;
const FEE_CHARGED_TITLE: &str = "Transaction Fee Charged";
const RESOURCE_FEE_CHARGED_TITLE: &str = "Resource Fee Charged";
const INCLUSION_FEE_TITLE: &str = "Inclusion Fee";
const NON_REFUNDABLE_TITLE: &str = "Non-refundable Resource Fee";
const REFUNDABLE_TITLE: &str = "Refundable Resource Fee";
const MAX_FEE_TITLE: &str = "Max Fee Set";
const MAX_RESOURCE_FEE_TITLE: &str = "Max Resource Fee";

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let resp = self.args.fetch_transaction(global_args).await?;
        let fee_table = FeeTable::new_from_transaction_response(&resp)?;
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
    pub fee_charged: i64,
    pub resource_fee_charged: i64,
    pub inclusion_fee_charged: i64,
    pub non_refundable_resource_fee_charged: i64,
    pub refundable_resource_fee_charged: i64,
    pub max_fee: i64,
    pub max_resource_fee: i64,
}

impl FeeTable {
    fn new_from_transaction_response(resp: &GetTransactionResponse) -> Result<Self, Error> {
        let tx_result = resp.result.clone().ok_or(Error::None {
            field: "tx_result".to_string(),
        })?; // fee charged
        let tx_meta = resp.result_meta.clone().ok_or(Error::None {
            field: "tx_meta".to_string(),
        })?; // resource fees
        let tx_envelope = resp.envelope.clone().ok_or(Error::None {
            field: "tx_envelope".to_string(),
        })?; // max fees

        let fee_charged = tx_result.fee_charged;
        let (non_refundable_resource_fee_charged, refundable_resource_fee_charged) =
            Self::resource_fees_charged(&tx_meta);

        let (max_fee, max_resource_fee) = Self::max_fees(&tx_envelope);

        let resource_fee_charged =
            non_refundable_resource_fee_charged + refundable_resource_fee_charged;
        let inclusion_fee_charged = fee_charged - resource_fee_charged;
        Ok(FeeTable {
            fee_charged,
            resource_fee_charged,
            inclusion_fee_charged,
            non_refundable_resource_fee_charged,
            refundable_resource_fee_charged,
            max_fee,
            max_resource_fee,
        })
    }

    fn max_fees(tx_envelope: &TransactionEnvelope) -> (i64, i64) {
        match tx_envelope {
            TransactionEnvelope::TxV0(transaction_v0_envelope) => {
                let fee = transaction_v0_envelope.tx.fee;
                (fee.into(), DEFAULT_FEE_VALUE)
            }
            TransactionEnvelope::Tx(transaction_v1_envelope) => {
                let tx = transaction_v1_envelope.tx.clone();
                let fee = tx.fee;
                let resource_fee = match tx.ext {
                    xdr::TransactionExt::V0 => DEFAULT_FEE_VALUE,
                    xdr::TransactionExt::V1(soroban_transaction_data) => {
                        soroban_transaction_data.resource_fee
                    }
                };

                (fee.into(), resource_fee)
            }
            TransactionEnvelope::TxFeeBump(fee_bump_transaction_envelope) => {
                let fee = fee_bump_transaction_envelope.tx.fee;
                (fee, DEFAULT_FEE_VALUE)
            }
        }
    }

    fn resource_fees_charged(tx_meta: &TransactionMeta) -> (i64, i64) {
        let (non_refundable_resource_fee_charged, refundable_resource_fee_charged) =
            match tx_meta.clone() {
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

        (
            non_refundable_resource_fee_charged,
            refundable_resource_fee_charged,
        )
    }

    fn should_include_resource_fees(&self) -> bool {
        self.resource_fee_charged != 0 || self.max_resource_fee != 0
    }

    fn print(&self) {
        let mut table = Table::new();
        table.set_format(Self::table_format());

        table.add_row(Row::new(vec![Cell::new(&format!(
            "{FEE_CHARGED_TITLE}: {}",
            self.fee_charged
        ))
        .style_spec("b")
        .with_hspan(3)]));

        if self.should_include_resource_fees() {
            table.add_row(Row::new(vec![
                Cell::new(&format!(
                    "{}: {}",
                    RESOURCE_FEE_CHARGED_TITLE, self.resource_fee_charged
                ))
                .style_spec("FY")
                .with_hspan(2),
                Cell::new(&format!(
                    "{INCLUSION_FEE_TITLE}: {}",
                    self.inclusion_fee_charged
                )),
            ]));

            table.add_row(Row::new(vec![
                Cell::new(&format!(
                    "{NON_REFUNDABLE_TITLE}: {}\n\ncalculated based on tx.v1.sorobanData.resources.*\n\ninstructions\nread\nwrite\nbandwidth (size of tx)",
                    self.non_refundable_resource_fee_charged
                ))
                .style_spec("FY"),
                Cell::new(&format!(
                    "{REFUNDABLE_TITLE}: {}\n\n\n\nrent\nevents\nreturn value",
                    self.refundable_resource_fee_charged
                ))
                .style_spec("FY"),
                Cell::new(&format!("{INCLUSION_FEE_TITLE}: {}", self.inclusion_fee_charged)),
            ]));
        }

        table.add_row(Row::new(vec![Cell::new(&format!(
            "{MAX_FEE_TITLE}: {}",
            self.max_fee
        ))
        .style_spec("FY")
        .with_hspan(3)]));

        if self.should_include_resource_fees() {
            table.add_row(Row::new(vec![
                Cell::new(&format!(
                    "{MAX_RESOURCE_FEE_TITLE}: {}",
                    self.max_resource_fee
                ))
                .style_spec("FY")
                .with_hspan(2),
                Cell::new(&format!(
                    "{INCLUSION_FEE_TITLE}: {}",
                    self.inclusion_fee_charged
                )),
            ]));
        }

        table.printstd();
    }

    fn table_format() -> TableFormat {
        FormatBuilder::new()
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
            .build()
    }
}

#[cfg(test)]
mod test {
    use soroban_rpc::GetTransactionResponse;

    use super::*;

    #[test]
    fn soroban_tx_fee_table() {
        let resp = soroban_tx_response().unwrap();
        let fee_table = FeeTable::new_from_transaction_response(&resp).unwrap();

        let expected_fee_charged = 185_119;
        let expected_non_refundable_charged = 59_343;
        let expected_refundable_charged = 125_676;
        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;
        let expected_max_fee = 248_869;
        let expected_max_resource_fee = 248_769;

        assert_eq!(fee_table.fee_charged, expected_fee_charged);
        assert_eq!(
            fee_table.resource_fee_charged,
            expected_resource_fee_charged
        );
        assert_eq!(
            fee_table.non_refundable_resource_fee_charged,
            expected_non_refundable_charged
        );
        assert_eq!(
            fee_table.refundable_resource_fee_charged,
            expected_refundable_charged
        );
        assert_eq!(
            fee_table.inclusion_fee_charged,
            expected_inclusion_fee_charged
        );
        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);
    }

    #[test]
    fn classic_tx_fee_table() {
        let resp = classic_tx_response().unwrap();
        let fee_table = FeeTable::new_from_transaction_response(&resp).unwrap();

        let expected_fee_charged = 100;
        let expected_non_refundable_charged = DEFAULT_FEE_VALUE;
        let expected_refundable_charged = DEFAULT_FEE_VALUE;
        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;
        let expected_max_fee = 100;
        let expected_max_resource_fee = DEFAULT_FEE_VALUE;

        assert_eq!(fee_table.fee_charged, expected_fee_charged);
        assert_eq!(
            fee_table.resource_fee_charged,
            expected_resource_fee_charged
        );
        assert_eq!(
            fee_table.non_refundable_resource_fee_charged,
            expected_non_refundable_charged
        );
        assert_eq!(
            fee_table.refundable_resource_fee_charged,
            expected_refundable_charged
        );
        assert_eq!(
            fee_table.inclusion_fee_charged,
            expected_inclusion_fee_charged
        );
        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);
    }

    #[test]
    fn fee_bump_tx_fee_table() {
        let resp = fee_bump_tx_response().unwrap();
        let fee_table = FeeTable::new_from_transaction_response(&resp).unwrap();

        let expected_fee_charged = 200;
        let expected_non_refundable_charged = DEFAULT_FEE_VALUE;
        let expected_refundable_charged = DEFAULT_FEE_VALUE;
        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;
        let expected_max_fee = 400;
        let expected_max_resource_fee = DEFAULT_FEE_VALUE;

        assert_eq!(fee_table.fee_charged, expected_fee_charged);
        assert_eq!(
            fee_table.resource_fee_charged,
            expected_resource_fee_charged
        );
        assert_eq!(
            fee_table.non_refundable_resource_fee_charged,
            expected_non_refundable_charged
        );
        assert_eq!(
            fee_table.refundable_resource_fee_charged,
            expected_refundable_charged
        );
        assert_eq!(
            fee_table.inclusion_fee_charged,
            expected_inclusion_fee_charged
        );
        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);
    }

    fn soroban_tx_response() -> Result<GetTransactionResponse, serde_json::Error> {
        serde_json::from_str(SOROBAN_TX_RESPONSE)
    }

    fn classic_tx_response() -> Result<GetTransactionResponse, serde_json::Error> {
        serde_json::from_str(CLASSIC_TX_RESPONSE)
    }

    fn fee_bump_tx_response() -> Result<GetTransactionResponse, serde_json::Error> {
        serde_json::from_str(FEE_BUMP_TX_RESPONSE)
    }

    const SOROBAN_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx":{"tx":{"source_account":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","fee":248869,"seq_num":197568495619,"cond":"none","memo":"none","operations":[{"source_account":null,"body":{"invoke_host_function":{"host_function":{"invoke_contract":{"contract_address":"CDJJ2YDDNWVVY6AFSN2UFLMG33Z2IE2ZVYCLU4FFEAVCICLF62IXO44D","function_name":"inc","args":[]}},"auth":[]}}}],"ext":{"v1":{"ext":"v0","resources":{"footprint":{"read_only":[{"contract_data":{"contract":"CDJJ2YDDNWVVY6AFSN2UFLMG33Z2IE2ZVYCLU4FFEAVCICLF62IXO44D","key":"ledger_key_contract_instance","durability":"persistent"}},{"contract_code":{"hash":"e54e59e63d77364714a001d2c968e811d8eafe96b725781458fb8b21acf6d50e"}}],"read_write":[{"contract_data":{"contract":"CDJJ2YDDNWVVY6AFSN2UFLMG33Z2IE2ZVYCLU4FFEAVCICLF62IXO44D","key":{"symbol":"COUNTER"},"durability":"persistent"}}]},"instructions":2092625,"read_bytes":7928,"write_bytes":80},"resource_fee":248769}}},"signatures":[{"hint":"d110e61c","signature":"6b2d9fba82e01a84129582815c554f7b158ea678aeb0eceaa596f21640e1ee926441c2ffebb4eeba81ac48b3c021e8435b6b6c61071a61e498aadcb4e7a04b08"}]}},"result":{"fee_charged":185119,"result":{"tx_success":[{"op_inner":{"invoke_host_function":{"success":"df071a249d03fc2f22313f75c734a254bbea03124cea77001704db0670b2fc02"}}}]},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":54,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988036662,"seq_num":197568495618,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":51,"seq_time":1750166268}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":54,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988036662,"seq_num":197568495619,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":54,"seq_time":1750166271}}}}}}}},"ext":"v0"}}],"operations":[{"changes":[{"created":{"last_modified_ledger_seq":54,"data":{"ttl":{"key_hash":"5ac6e64993239b0eba43a13feecffb99ffdbd359624949f275cc0aa417fc75bd","live_until_ledger_seq":2073653}},"ext":"v0"}},{"created":{"last_modified_ledger_seq":54,"data":{"contract_data":{"ext":"v0","contract":"CDJJ2YDDNWVVY6AFSN2UFLMG33Z2IE2ZVYCLU4FFEAVCICLF62IXO44D","key":{"symbol":"COUNTER"},"durability":"persistent","val":{"u32":1}}},"ext":"v0"}}]}],"tx_changes_after":[{"state":{"last_modified_ledger_seq":54,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988036662,"seq_num":197568495619,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":54,"seq_time":1750166271}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":54,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988100412,"seq_num":197568495619,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":54,"seq_time":1750166271}}}}}}}},"ext":"v0"}}],"soroban_meta":{"ext":{"v1":{"ext":"v0","total_non_refundable_resource_fee_charged":59343,"total_refundable_resource_fee_charged":125676,"rent_fee_charged":125597}},"events":[],"return_value":{"u32":1},"diagnostic_events":[{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"d29d60636dab5c7805937542ad86def3a41359ae04ba70a5202a240965f69177"},{"symbol":"inc"}],"data":"void"}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"d29d60636dab5c7805937542ad86def3a41359ae04ba70a5202a240965f69177","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"log"}],"data":{"vec":[{"string":"count: {}"},{"u32":0}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"d29d60636dab5c7805937542ad86def3a41359ae04ba70a5202a240965f69177","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_return"},{"symbol":"inc"}],"data":{"u32":1}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_entry"}],"data":{"u64":3}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_entry"}],"data":{"u64":1}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_read_byte"}],"data":{"u64":7928}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_write_byte"}],"data":{"u64":80}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_key_byte"}],"data":{"u64":144}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_key_byte"}],"data":{"u64":0}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_data_byte"}],"data":{"u64":104}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_data_byte"}],"data":{"u64":80}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_code_byte"}],"data":{"u64":7824}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_code_byte"}],"data":{"u64":0}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event"}],"data":{"u64":0}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event_byte"}],"data":{"u64":0}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"cpu_insn"}],"data":{"u64":2000326}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"mem_byte"}],"data":{"u64":1487598}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"invoke_time_nsecs"}],"data":{"u64":158917}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_key_byte"}],"data":{"u64":60}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_data_byte"}],"data":{"u64":104}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_code_byte"}],"data":{"u64":7824}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_emit_event_byte"}],"data":{"u64":0}}}}}]}}}}"#;

    const CLASSIC_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx":{"tx":{"source_account":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","fee":100,"seq_num":197568495620,"cond":"none","memo":"none","operations":[{"source_account":null,"body":{"manage_data":{"data_name":"test","data_value":"abcdef"}}}],"ext":"v0"},"signatures":[{"hint":"d110e61c","signature":"89be4c9f86a3aa19de242f54b69e95894c451f0fa6e8c6f7ad7bb353e08c6aefafb45e73746340e54f87aa9112aee2e7424b81289e0a7756c3e024406d0cdf0a"}]}},"result":{"fee_charged":100,"result":{"tx_success":[{"op_inner":{"manage_data":"success"}}]},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":15678,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988100312,"seq_num":197568495619,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":54,"seq_time":1750166271}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":15678,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988100312,"seq_num":197568495620,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":15678,"seq_time":1750185606}}}}}}}},"ext":"v0"}}],"operations":[{"changes":[{"created":{"last_modified_ledger_seq":15678,"data":{"data":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","data_name":"test","data_value":"abcdef","ext":"v0"}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":15678,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988100312,"seq_num":197568495620,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":15678,"seq_time":1750185606}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":15678,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988100312,"seq_num":197568495620,"num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":15678,"seq_time":1750185606}}}}}}}},"ext":"v0"}}]}],"tx_changes_after":[],"soroban_meta":null}}}"#;

    const FEE_BUMP_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx_fee_bump":{"tx":{"fee_source":"GDBFMEGF2EVTNISNTYVOOYGXAEP5A353YJCPDRGUH3L6GMIDATR4BWY6","fee":400,"inner_tx":{"tx":{"tx":{"source_account":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","fee":100,"seq_num":197568495621,"cond":{"time":{"min_time":0,"max_time":1750189754}},"memo":"none","operations":[{"source_account":null,"body":{"payment":{"destination":"GDN2KA5HJ55DDXBKBBAPGWPXXAWEMLSPZIH3B2G4N7ERRUL7SN5BIRAX","asset":"native","amount":100000000}}}],"ext":"v0"},"signatures":[{"hint":"d110e61c","signature":"cfc3a142055e944995dcb1246bdbe84cd3f834ce8ae7a26ceeaeaa4edaee168628ed50d285d111d632d983a71fc305056e87d50ca50c80c193ed580398f43a0c"}]}},"ext":"v0"},"signatures":[{"hint":"0304e3c0","signature":"cc4d0d4a92ac2dbaeb7724c01713b4428ce48f61d263a70493e04cb9fadccd25342775200b1e1c27719971ce0f98ce44d0e2655491d432938aa22c92bf64df07"}]}},"result":{"fee_charged":200,"result":{"tx_fee_bump_inner_success":{"transaction_hash":"95a59fd736d69126705692a877c78881a33c28fc50684e61189b6b8bfa02e646","result":{"fee_charged":100,"result":{"tx_success":[{"op_inner":{"payment":"success"}}]},"ext":"v0"}}},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":18731,"data":{"account":{"account_id":"GDBFMEGF2EVTNISNTYVOOYGXAEP5A353YJCPDRGUH3L6GMIDATR4BWY6","balance":99999999800,"seq_num":80427557584896,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":18731,"data":{"account":{"account_id":"GDBFMEGF2EVTNISNTYVOOYGXAEP5A353YJCPDRGUH3L6GMIDATR4BWY6","balance":99999999800,"seq_num":80427557584896,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":15678,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988100312,"seq_num":197568495620,"num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":15678,"seq_time":1750185606}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":18731,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988100312,"seq_num":197568495621,"num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":18731,"seq_time":1750189724}}}}}}}},"ext":"v0"}}],"operations":[{"changes":[{"state":{"last_modified_ledger_seq":18731,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99988100312,"seq_num":197568495621,"num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":18731,"seq_time":1750189724}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":18731,"data":{"account":{"account_id":"GCBVAIKUZELFVCV6S7KBS47SF2DQQXTN63TJM7D3CNZ7PD6RCDTBYULI","balance":99888100312,"seq_num":197568495621,"num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":0,"selling":0},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":18731,"seq_time":1750189724}}}}}}}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":18687,"data":{"account":{"account_id":"GDN2KA5HJ55DDXBKBBAPGWPXXAWEMLSPZIH3B2G4N7ERRUL7SN5BIRAX","balance":100000000000,"seq_num":80260053860352,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":18731,"data":{"account":{"account_id":"GDN2KA5HJ55DDXBKBBAPGWPXXAWEMLSPZIH3B2G4N7ERRUL7SN5BIRAX","balance":100100000000,"seq_num":80260053860352,"num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}}]}],"tx_changes_after":[],"soroban_meta":null}}}"#;
}
