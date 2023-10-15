use std::{fmt::Debug, path::Path, str::FromStr};

use clap::{command, Parser};
use soroban_env_host::xdr::{
    BumpFootprintExpirationOp, Error as XdrError, ExpirationEntry, ExtensionPoint, LedgerEntry,
    LedgerEntryChange, LedgerEntryData, LedgerFootprint, Memo, MuxedAccount, Operation,
    OperationBody, Preconditions, SequenceNumber, SorobanResources, SorobanTransactionData,
    Transaction, TransactionExt, TransactionMeta, TransactionMetaV3, Uint256,
};

use crate::{
    commands::config,
    key,
    rpc::{self, Client},
    wasm, Pwd,
};

const MAX_LEDGERS_TO_EXPIRE: u32 = 535_679;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Number of ledgers to extend the entries
    #[arg(long, required = true)]
    pub ledgers_to_expire: u32,

    /// Only print the new expiration ledger
    #[arg(long)]
    pub expiration_ledger_only: bool,

    #[command(flatten)]
    pub key: key::Args,

    #[command(flatten)]
    pub config: config::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
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

    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("either `--key` or `--key-xdr` are required")]
    KeyIsRequired,
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("Ledger entry not found")]
    LedgerEntryNotFound,
    #[error("missing operation result")]
    MissingOperationResult,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error(transparent)]
    Key(#[from] key::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self) -> Result<(), Error> {
        let expiration_ledger_seq = if self.config.is_no_network() {
            self.run_in_sandbox()?
        } else {
            self.run_against_rpc_server().await?
        };
        if self.expiration_ledger_only {
            println!("{expiration_ledger_seq}");
        } else {
            println!("New expiration ledger: {expiration_ledger_seq}");
        }

        Ok(())
    }

    fn ledgers_to_expire(&self) -> u32 {
        let res = u32::min(self.ledgers_to_expire, MAX_LEDGERS_TO_EXPIRE);
        if res < self.ledgers_to_expire {
            tracing::warn!(
                "Ledgers to expire is too large, using max value of {MAX_LEDGERS_TO_EXPIRE}"
            );
        }
        res
    }

    async fn run_against_rpc_server(&self) -> Result<u32, Error> {
        let network = self.config.get_network()?;
        tracing::trace!(?network);
        let keys = self.key.parse_keys()?;
        let network = &self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        let key = self.config.key_pair()?;
        let ledgers_to_expire = self.ledgers_to_expire();

        // Get the account sequence number
        let public_strkey =
            stellar_strkey::ed25519::PublicKey(key.verifying_key().to_bytes()).to_string();
        let account_details = client.get_account(&public_strkey).await?;
        let sequence: i64 = account_details.seq_num.into();

        let tx = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(key.verifying_key().to_bytes())),
            fee: self.fee.fee,
            seq_num: SequenceNumber(sequence + 1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::BumpFootprintExpiration(BumpFootprintExpirationOp {
                    ext: ExtensionPoint::V0,
                    ledgers_to_expire,
                }),
            }]
            .try_into()?,
            ext: TransactionExt::V1(SorobanTransactionData {
                ext: ExtensionPoint::V0,
                resources: SorobanResources {
                    footprint: LedgerFootprint {
                        read_only: keys.clone().try_into()?,
                        read_write: vec![].try_into()?,
                    },
                    instructions: 0,
                    read_bytes: 0,
                    write_bytes: 0,
                },
                refundable_fee: 0,
            }),
        };

        let (result, meta, events) = client
            .prepare_and_send_transaction(&tx, &key, &[], &network.network_passphrase, None, None)
            .await?;

        tracing::trace!(?result);
        tracing::trace!(?meta);
        if !events.is_empty() {
            tracing::info!("Events:\n {events:#?}");
        }

        // The transaction from core will succeed regardless of whether it actually found & bumped
        // the entry, so we have to inspect the result meta to tell if it worked or not.
        let TransactionMeta::V3(TransactionMetaV3 { operations, .. }) = meta else {
            return Err(Error::LedgerEntryNotFound);
        };

        // Simply check if there is exactly one entry here. We only support bumping a single
        // entry via this command (which we should fix separately, but).
        if operations.len() == 0 {
            return Err(Error::LedgerEntryNotFound);
        }

        if operations[0].changes.is_empty() {
            let entry = client.get_full_ledger_entries(&keys).await?;
            let expire = entry.entries[0].expiration_ledger_seq;
            if entry.latest_ledger + i64::from(ledgers_to_expire) < i64::from(expire) {
                return Ok(expire);
            }
        }

        match (&operations[0].changes[0], &operations[0].changes[1]) {
            (
                LedgerEntryChange::State(_),
                LedgerEntryChange::Updated(LedgerEntry {
                    data:
                        LedgerEntryData::Expiration(ExpirationEntry {
                            expiration_ledger_seq,
                            ..
                        }),
                    ..
                }),
            ) => Ok(*expiration_ledger_seq),
            _ => Err(Error::LedgerEntryNotFound),
        }
    }

    fn run_in_sandbox(&self) -> Result<u32, Error> {
        let keys = self.key.parse_keys()?;

        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let mut state = self.config.get_state()?;

        // Update all matching entries
        let mut expiration_ledger_seq = None;
        state.ledger_entries = state
            .ledger_entries
            .iter()
            .map(|(k, v)| {
                let new_k = k.as_ref().clone();
                let new_v = v.0.as_ref().clone();
                let new_e = v.1;
                (
                    Box::new(new_k.clone()),
                    (
                        Box::new(new_v),
                        if keys.contains(&new_k) {
                            // It must have an expiration since it's a contract data entry
                            let old_expiration = v.1.unwrap();
                            expiration_ledger_seq = Some(old_expiration + self.ledgers_to_expire);
                            expiration_ledger_seq
                        } else {
                            new_e
                        },
                    ),
                )
            })
            .collect::<Vec<_>>();

        self.config.set_state(&state)?;

        let Some(new_expiration_ledger_seq) = expiration_ledger_seq else {
            return Err(Error::LedgerEntryNotFound);
        };

        Ok(new_expiration_ledger_seq)
    }
}
