use std::array::TryFromSliceError;
use std::fmt::Debug;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};

use crate::xdr::{
    self, ContractCodeEntryExt, Error as XdrError, Hash, HostFunction, InvokeHostFunctionOp,
    LedgerEntryData, Limits, OperationBody, ReadXdr, ScMetaEntry, ScMetaV0, Transaction,
    TransactionResult, TransactionResultResult, VecM, WriteXdr,
};
use clap::Parser;

use super::{build, restore};
use crate::commands::tx::fetch;
use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::{
        global,
        txn_result::{TxnEnvelopeResult, TxnResult},
    },
    config::{self, data, network},
    key,
    print::Print,
    rpc,
    tx::builder::{self, TxExt},
    utils, wasm,
};

const CONTRACT_META_SDK_KEY: &str = "rssdkver";
const PUBLIC_NETWORK_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config: config::Args,

    #[command(flatten)]
    pub resources: crate::resources::Args,

    /// Path to wasm binary. When omitted, builds the project automatically.
    #[arg(long)]
    pub wasm: Option<PathBuf>,

    #[arg(long, short = 'i', default_value = "false")]
    /// Whether to ignore safety checks when deploying contracts
    pub ignore_checks: bool,

    /// Build the transaction and only write the base64 xdr to stdout
    #[arg(long)]
    pub build_only: bool,

    /// Package to build when --wasm is not provided
    #[arg(long, help_heading = "Build Options")]
    pub package: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),

    #[error("internal conversion error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),

    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),

    #[error("jsonrpc error: {0}")]
    JsonRpc(#[from] jsonrpsee_core::Error),

    #[error(transparent)]
    Rpc(#[from] rpc::Error),

    #[error(transparent)]
    Config(#[from] config::Error),

    #[error(transparent)]
    Wasm(#[from] wasm::Error),

    #[error("unexpected ({length}) simulate transaction result length")]
    UnexpectedSimulateTransactionResultSize { length: usize },

    #[error(transparent)]
    Restore(#[from] restore::Error),

    #[error("cannot parse WASM file {wasm}: {error}")]
    CannotParseWasm {
        wasm: std::path::PathBuf,
        error: wasm::Error,
    },

    #[error("the deployed smart contract {wasm} was built with Soroban Rust SDK v{version}, a release candidate version not intended for use with the Stellar Public Network. To deploy anyway, use --ignore-checks")]
    ContractCompiledWithReleaseCandidateSdk {
        wasm: std::path::PathBuf,
        version: String,
    },

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Data(#[from] data::Error),

    #[error(transparent)]
    Builder(#[from] builder::Error),

    #[error(transparent)]
    Fee(#[from] fetch::fee::Error),

    #[error(transparent)]
    Fetch(#[from] fetch::Error),

    #[error(transparent)]
    Build(#[from] build::Error),

    #[error("no buildable contracts found in workspace (no packages with crate-type cdylib)")]
    NoBuildableContracts,

    #[error("--wasm flag is required when not in a Cargo project")]
    WasmNotProvided,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let wasm_paths = self.resolve_wasm_paths(global_args)?;
        for wasm_path in &wasm_paths {
            let res = self
                .upload_wasm(
                    wasm_path,
                    &self.config,
                    global_args.quiet,
                    global_args.no_cache,
                )
                .await?
                .to_envelope();
            match res {
                TxnEnvelopeResult::TxnEnvelope(tx) => {
                    println!("{}", tx.to_xdr_base64(Limits::none())?);
                }
                TxnEnvelopeResult::Res(hash) => println!("{}", hex::encode(hash)),
            }
        }
        Ok(())
    }

    /// Programmatic API for uploading a single WASM file.
    /// Expects `self.wasm` to be set. Used by deploy command internally.
    #[allow(clippy::too_many_lines)]
    #[allow(unused_variables)]
    pub async fn execute(
        &self,
        config: &config::Args,
        quiet: bool,
        no_cache: bool,
    ) -> Result<TxnResult<Hash>, Error> {
        let wasm_path = self.wasm.clone().ok_or(Error::WasmNotProvided)?;
        self.upload_wasm(&wasm_path, config, quiet, no_cache).await
    }

    fn resolve_wasm_paths(&self, global_args: &global::Args) -> Result<Vec<PathBuf>, Error> {
        if let Some(wasm) = &self.wasm {
            Ok(vec![wasm.clone()])
        } else {
            let build_cmd = build::Cmd {
                package: self.package.clone(),
                ..build::Cmd::default()
            };
            let contracts = build_cmd.run(global_args)?;
            if contracts.is_empty() {
                return Err(Error::NoBuildableContracts);
            }
            Ok(contracts.into_iter().map(|c| c.path).collect())
        }
    }

    #[allow(clippy::too_many_lines)]
    #[allow(unused_variables)]
    async fn upload_wasm(
        &self,
        wasm_path: &Path,
        config: &config::Args,
        quiet: bool,
        no_cache: bool,
    ) -> Result<TxnResult<Hash>, Error> {
        let print = Print::new(quiet);
        let wasm_path = wasm_path.to_path_buf();
        let wasm_args = wasm::Args {
            wasm: wasm_path.clone(),
        };
        let contract = wasm_args.read()?;
        let network = config.get_network()?;
        let client = network.rpc_client()?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;
        let wasm_spec = &wasm_args.parse().map_err(|e| Error::CannotParseWasm {
            wasm: wasm_path.clone(),
            error: e,
        })?;

        // Check Rust SDK version if using the public network.
        if let Some(rs_sdk_ver) = get_contract_meta_sdk_version(wasm_spec) {
            if rs_sdk_ver.contains("rc")
                && !self.ignore_checks
                && network.network_passphrase == PUBLIC_NETWORK_PASSPHRASE
            {
                return Err(Error::ContractCompiledWithReleaseCandidateSdk {
                    wasm: wasm_path.clone(),
                    version: rs_sdk_ver,
                });
            } else if rs_sdk_ver.contains("rc")
                && network.network_passphrase == PUBLIC_NETWORK_PASSPHRASE
            {
                tracing::warn!("the deployed smart contract {path} was built with Soroban Rust SDK v{rs_sdk_ver}, a release candidate version not intended for use with the Stellar Public Network", path = wasm_path.display());
            }
        }

        // Get the account sequence number
        let source_account = config.source_account().await?;

        let account_details = client
            .get_account(&source_account.clone().to_string())
            .await?;
        let sequence: i64 = account_details.seq_num.into();

        let (tx_without_preflight, hash) = build_install_contract_code_tx(
            &contract,
            sequence + 1,
            config.get_inclusion_fee()?,
            &source_account,
        )?;

        if self.build_only {
            return Ok(TxnResult::Txn(Box::new(tx_without_preflight)));
        }

        let should_check = true;

        if should_check {
            let code_key =
                xdr::LedgerKey::ContractCode(xdr::LedgerKeyContractCode { hash: hash.clone() });
            let contract_data = client.get_ledger_entries(&[code_key]).await?;

            // Skip install if the contract is already installed, and the contract has an extension version that isn't V0.
            // In protocol 21 extension V1 was added that stores additional information about a contract making execution
            // of the contract cheaper. So if folks want to reinstall we should let them which is why the install will still
            // go ahead if the contract has a V0 extension.
            if let Some(entries) = contract_data.entries {
                if let Some(entry_result) = entries.first() {
                    let entry: LedgerEntryData =
                        LedgerEntryData::from_xdr_base64(&entry_result.xdr, Limits::none())?;

                    match &entry {
                        LedgerEntryData::ContractCode(code) => {
                            // Skip reupload if this isn't V0 because V1 extension already
                            // exists.
                            if code.ext.ne(&ContractCodeEntryExt::V0) {
                                print.infoln("Skipping install because wasm already installed");
                                return Ok(TxnResult::Res(hash));
                            }
                        }
                        _ => {
                            tracing::warn!("Entry retrieved should be of type ContractCode");
                        }
                    }
                }
            }
        }

        print.infoln("Simulating install transaction…");

        let assembled = simulate_and_assemble_transaction(
            &client,
            &tx_without_preflight,
            self.resources.resource_config(),
            self.resources.resource_fee,
        )
        .await?;
        let assembled = self.resources.apply_to_assembled_txn(assembled);
        let txn = Box::new(assembled.transaction().clone());
        let signed_txn = &self.config.sign(*txn, quiet).await?;

        print.globeln("Submitting install transaction…");
        let txn_resp = client.send_transaction_polling(signed_txn).await?;
        self.resources.print_cost_info(&txn_resp)?;

        if !no_cache {
            data::write(txn_resp.clone().try_into().unwrap(), &network.rpc_uri()?)?;
        }

        // Currently internal errors are not returned if the contract code is expired
        if let Some(TransactionResult {
            result: TransactionResultResult::TxInternalError,
            ..
        }) = txn_resp.result
        {
            // Now just need to restore it and don't have to install again
            restore::Cmd {
                key: key::Args {
                    contract_id: None,
                    key: None,
                    key_xdr: None,
                    wasm: Some(wasm_path.clone()),
                    wasm_hash: None,
                    durability: super::Durability::Persistent,
                },
                config: config.clone(),
                resources: self.resources.clone(),
                ledgers_to_extend: None,
                ttl_ledger_only: true,
                build_only: self.build_only,
            }
            .execute(config, quiet, no_cache)
            .await?;
        }

        if !no_cache {
            data::write_spec(&hash.to_string(), &wasm_spec.spec)?;
        }

        Ok(TxnResult::Res(hash))
    }
}

fn get_contract_meta_sdk_version(wasm_spec: &soroban_spec_tools::contract::Spec) -> Option<String> {
    let rs_sdk_version_option = if let Some(_meta) = &wasm_spec.meta_base64 {
        wasm_spec.meta.iter().find(|entry| match entry {
            ScMetaEntry::ScMetaV0(ScMetaV0 { key, .. }) => {
                key.to_utf8_string_lossy().contains(CONTRACT_META_SDK_KEY)
            }
        })
    } else {
        None
    };

    if let Some(rs_sdk_version_entry) = &rs_sdk_version_option {
        match rs_sdk_version_entry {
            ScMetaEntry::ScMetaV0(ScMetaV0 { val, .. }) => {
                return Some(val.to_utf8_string_lossy());
            }
        }
    }

    None
}

pub(crate) fn build_install_contract_code_tx(
    source_code: &[u8],
    sequence: i64,
    fee: u32,
    source: &xdr::MuxedAccount,
) -> Result<(Transaction, Hash), Error> {
    let hash = utils::contract_hash(source_code)?;

    let op = xdr::Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::UploadContractWasm(source_code.try_into()?),
            auth: VecM::default(),
        }),
    };
    let tx = Transaction::new_tx(source.clone(), fee, sequence, op);

    Ok((tx, hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_install_contract_code() {
        let result = build_install_contract_code_tx(
            b"foo",
            300,
            1,
            &stellar_strkey::ed25519::PublicKey::from_payload(
                utils::parse_secret_key("SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP")
                    .unwrap()
                    .verifying_key()
                    .as_bytes(),
            )
            .unwrap()
            .to_string()
            .parse()
            .unwrap(),
        );

        assert!(result.is_ok());
    }
}
