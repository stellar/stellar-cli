use std::{fmt::Debug, path::Path, str::FromStr};

use crate::{
    log::extract_events,
    tx::sim_sign_and_send_tx,
    xdr::{
        Error as XdrError, ExtensionPoint, LedgerEntry, LedgerEntryChange, LedgerEntryData,
        LedgerFootprint, Limits, Memo, Operation, OperationBody, Preconditions, RestoreFootprintOp,
        SequenceNumber, SorobanResources, SorobanTransactionData, SorobanTransactionDataExt,
        Transaction, TransactionExt, TransactionMeta, TransactionMetaV3, TransactionMetaV4,
        TtlEntry, WriteXdr,
    },
};
use clap::Parser;
use stellar_strkey::DecodeError;

use crate::commands::tx::fetch;
use crate::{
    commands::{
        contract::extend,
        global,
        txn_result::{TxnEnvelopeResult, TxnResult},
    },
    config::{self, data, locator, network},
    key, rpc, wasm, Pwd,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub key: key::Args,

    /// Number of ledgers to extend the entry
    #[arg(long)]
    pub ledgers_to_extend: Option<u32>,

    /// Only print the new Time To Live ledger
    #[arg(long)]
    pub ttl_ledger_only: bool,

    #[command(flatten)]
    pub config: config::Args,

    #[command(flatten)]
    pub resources: crate::resources::Args,

    /// Build the transaction and only write the base64 xdr to stdout
    #[arg(long)]
    pub build_only: bool,
}

impl FromStr for Cmd {
    type Err = clap::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::{CommandFactory, FromArgMatches};
        Self::from_arg_matches_mut(&mut Self::command().get_matches_from(s.split_whitespace()))
    }
}

impl Pwd for Cmd {
    fn set_pwd(&mut self, pwd: &Path) {
        self.config.set_pwd(pwd);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing key {key}: {error}")]
    CannotParseKey {
        key: String,
        error: soroban_spec_tools::Error,
    },

    #[error("parsing XDR key {key}: {error}")]
    CannotParseXdrKey { key: String, error: XdrError },

    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),

    #[error(transparent)]
    Config(#[from] config::Error),

    #[error("either `--key` or `--key-xdr` are required")]
    KeyIsRequired,

    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),

    #[error("Ledger entry not found")]
    LedgerEntryNotFound,

    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error("missing operation result")]
    MissingOperationResult,

    #[error(transparent)]
    Rpc(#[from] rpc::Error),

    #[error(transparent)]
    Wasm(#[from] wasm::Error),

    #[error(transparent)]
    Key(#[from] key::Error),

    #[error(transparent)]
    Extend(#[from] extend::Error),

    #[error(transparent)]
    Data(#[from] data::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Fee(#[from] fetch::fee::Error),

    #[error(transparent)]
    Fetch(#[from] fetch::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let res = self
            .execute(&self.config, global_args.quiet, global_args.no_cache)
            .await?
            .to_envelope();
        let expiration_ledger_seq = match res {
            TxnEnvelopeResult::TxnEnvelope(tx) => {
                println!("{}", tx.to_xdr_base64(Limits::none())?);
                return Ok(());
            }
            TxnEnvelopeResult::Res(res) => res,
        };
        if let Some(ledgers_to_extend) = self.ledgers_to_extend {
            extend::Cmd {
                key: self.key.clone(),
                ledgers_to_extend,
                config: self.config.clone(),
                resources: self.resources.clone(),
                ttl_ledger_only: false,
                build_only: self.build_only,
            }
            .run(global_args)
            .await?;
        } else {
            println!("New ttl ledger: {expiration_ledger_seq}");
        }

        Ok(())
    }

    pub async fn execute(
        &self,
        config: &config::Args,
        quiet: bool,
        no_cache: bool,
    ) -> Result<TxnResult<u32>, Error> {
        let print = crate::print::Print::new(quiet);
        let network = config.get_network()?;
        tracing::trace!(?network);
        let entry_keys = self.key.parse_keys(&config.locator, &network)?;
        let client = network.rpc_client()?;
        let source_account = config.source_account().await?;

        // Get the account sequence number
        let account_details = client
            .get_account(&source_account.clone().to_string())
            .await?;
        let sequence: i64 = account_details.seq_num.into();

        let tx = Box::new(Transaction {
            source_account,
            fee: config.get_inclusion_fee()?,
            seq_num: SequenceNumber(sequence + 1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::RestoreFootprint(RestoreFootprintOp {
                    ext: ExtensionPoint::V0,
                }),
            }]
            .try_into()?,
            ext: TransactionExt::V1(SorobanTransactionData {
                ext: SorobanTransactionDataExt::V0,
                resources: SorobanResources {
                    footprint: LedgerFootprint {
                        read_only: vec![].try_into()?,
                        read_write: entry_keys.clone().try_into()?,
                    },
                    instructions: self.resources.instructions.unwrap_or_default(),
                    disk_read_bytes: 0,
                    write_bytes: 0,
                },
                resource_fee: 0,
            }),
        });
        if self.build_only {
            return Ok(TxnResult::Txn(tx));
        }

        let res = sim_sign_and_send_tx::<Error>(
            &client,
            &tx,
            config,
            &self.resources,
            &[],
            quiet,
            no_cache,
        )
        .await?;

        let meta = res
            .result_meta
            .as_ref()
            .ok_or(Error::MissingOperationResult)?;

        tracing::trace!(?meta);

        let events = extract_events(meta);

        crate::log::event::all(&events);
        crate::log::event::contract(&events, &print);

        // The transaction from core will succeed regardless of whether it actually found &
        // restored the entry, so we have to inspect the result meta to tell if it worked or not.
        let changes = match meta {
            TransactionMeta::V4(TransactionMetaV4 { operations, .. }) => {
                // Simply check if there is exactly one entry here. We only support restoring a single
                // entry via this command (which we should fix separately, but).
                if operations.is_empty() {
                    return Err(Error::LedgerEntryNotFound);
                }

                operations[0].changes.clone()
            }
            TransactionMeta::V3(TransactionMetaV3 { operations, .. }) => {
                // Simply check if there is exactly one entry here. We only support restoring a single
                // entry via this command (which we should fix separately, but).
                if operations.is_empty() {
                    return Err(Error::LedgerEntryNotFound);
                }

                operations[0].changes.clone()
            }
            _ => return Err(Error::LedgerEntryNotFound),
        };
        tracing::debug!("Changes:\nlen:{}\n{changes:#?}", changes.len());

        if changes.is_empty() {
            print.infoln("No changes detected, transaction was a no-op.");
            let entry = client.get_full_ledger_entries(&entry_keys).await?;
            let extension = entry.entries[0].live_until_ledger_seq.unwrap_or_default();

            return Ok(TxnResult::Res(extension));
        }

        Ok(TxnResult::Res(
            parse_changes(&changes.to_vec()).ok_or(Error::LedgerEntryNotFound)?,
        ))
    }
}

fn parse_changes(changes: &[LedgerEntryChange]) -> Option<u32> {
    changes
        .iter()
        .filter_map(|change| match change {
            LedgerEntryChange::Restored(LedgerEntry {
                data:
                    LedgerEntryData::Ttl(TtlEntry {
                        live_until_ledger_seq,
                        ..
                    }),
                ..
            })
            | LedgerEntryChange::Updated(LedgerEntry {
                data:
                    LedgerEntryData::Ttl(TtlEntry {
                        live_until_ledger_seq,
                        ..
                    }),
                ..
            })
            | LedgerEntryChange::Created(LedgerEntry {
                data:
                    LedgerEntryData::Ttl(TtlEntry {
                        live_until_ledger_seq,
                        ..
                    }),
                ..
            }) => Some(*live_until_ledger_seq),
            _ => None,
        })
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        ContractDataDurability::Persistent, ContractDataEntry, ContractId, Hash, LedgerEntry,
        LedgerEntryChange, LedgerEntryData, ScAddress, ScSymbol, ScVal, SequenceNumber, StringM,
        TtlEntry,
    };

    #[test]
    fn test_parse_changes_two_changes_restored() {
        // Test the original expected format with 2 changes
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 12345,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![
            LedgerEntryChange::State(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry.clone()),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
            LedgerEntryChange::Restored(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
        ];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(12345));
    }

    #[test]
    fn test_parse_two_changes_that_had_expired() {
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 55555,
            key_hash: Hash([0; 32]),
        };

        let counter = "COUNTER".parse::<StringM<32>>().unwrap();
        let contract_data_entry = ContractDataEntry {
            ext: ExtensionPoint::default(),
            contract: ScAddress::Contract(ContractId(Hash([0; 32]))),
            key: ScVal::Symbol(ScSymbol(counter)),
            durability: Persistent,
            val: ScVal::U32(1),
        };

        let changes = vec![
            LedgerEntryChange::Restored(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry.clone()),
                last_modified_ledger_seq: 37429,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
            LedgerEntryChange::Restored(LedgerEntry {
                data: LedgerEntryData::ContractData(contract_data_entry.clone()),
                last_modified_ledger_seq: 37429,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
        ];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(55555));
    }

    #[test]
    fn test_parse_changes_two_changes_updated() {
        // Test the original expected format with 2 changes, but second change is Updated
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 67890,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![
            LedgerEntryChange::State(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry.clone()),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
            LedgerEntryChange::Updated(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
        ];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(67890));
    }

    #[test]
    fn test_parse_changes_two_changes_created() {
        // Test the original expected format with 2 changes, but second change is Created
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 11111,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![
            LedgerEntryChange::State(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry.clone()),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
            LedgerEntryChange::Created(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
        ];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(11111));
    }

    #[test]
    fn test_parse_changes_single_change_restored() {
        // Test the new single change format with Restored type
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 22222,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![LedgerEntryChange::Restored(LedgerEntry {
            data: LedgerEntryData::Ttl(ttl_entry),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        })];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(22222));
    }

    #[test]
    fn test_parse_changes_single_change_updated() {
        // Test the new single change format with Updated type
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 33333,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![LedgerEntryChange::Updated(LedgerEntry {
            data: LedgerEntryData::Ttl(ttl_entry),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        })];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(33333));
    }

    #[test]
    fn test_parse_changes_single_change_created() {
        // Test the new single change format with Created type
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 44444,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![LedgerEntryChange::Created(LedgerEntry {
            data: LedgerEntryData::Ttl(ttl_entry),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        })];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(44444));
    }

    #[test]
    fn test_parse_changes_invalid_two_changes() {
        // Test invalid 2-change format (not TTL data)
        let not_ttl_change = LedgerEntryChange::Restored(LedgerEntry {
            data: LedgerEntryData::Account(crate::xdr::AccountEntry {
                account_id: crate::xdr::AccountId(crate::xdr::PublicKey::PublicKeyTypeEd25519(
                    crate::xdr::Uint256([0; 32]),
                )),
                balance: 0,
                seq_num: SequenceNumber(0),
                num_sub_entries: 0,
                inflation_dest: None,
                flags: 0,
                home_domain: crate::xdr::String32::default(),
                thresholds: crate::xdr::Thresholds::default(),
                signers: crate::xdr::VecM::default(),
                ext: crate::xdr::AccountEntryExt::V0,
            }),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        });

        let changes = vec![not_ttl_change.clone(), not_ttl_change];
        let result = parse_changes(&changes);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_changes_invalid_single_change() {
        // Test invalid single change format (not TTL data)
        let changes = vec![LedgerEntryChange::Restored(LedgerEntry {
            data: LedgerEntryData::Account(crate::xdr::AccountEntry {
                account_id: crate::xdr::AccountId(crate::xdr::PublicKey::PublicKeyTypeEd25519(
                    crate::xdr::Uint256([0; 32]),
                )),
                balance: 0,
                seq_num: SequenceNumber(0),
                num_sub_entries: 0,
                inflation_dest: None,
                flags: 0,
                home_domain: crate::xdr::String32::default(),
                thresholds: crate::xdr::Thresholds::default(),
                signers: crate::xdr::VecM::default(),
                ext: crate::xdr::AccountEntryExt::V0,
            }),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        })];

        let result = parse_changes(&changes);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_changes_empty_changes() {
        // Test empty changes array
        let changes = vec![];

        let result = parse_changes(&changes);
        assert_eq!(result, None);
    }
}
