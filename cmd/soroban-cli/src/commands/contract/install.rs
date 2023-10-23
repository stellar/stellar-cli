use std::array::TryFromSliceError;
use std::fmt::Debug;
use std::num::ParseIntError;

use clap::{command, Parser};
use soroban_env_host::xdr::{
    Error as XdrError, Hash, HostFunction, InvokeHostFunctionOp, Memo, MuxedAccount, Operation,
    OperationBody, Preconditions, SequenceNumber, Transaction, TransactionExt, TransactionResult,
    TransactionResultResult, Uint256, VecM,
};

use super::restore;
use crate::key;
use crate::rpc::{self, Client};
use crate::{commands::config, utils, wasm};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config: config::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
    #[command(flatten)]
    pub wasm: wasm::Args,
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
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let res_str = hex::encode(self.run_and_get_hash().await?);
        println!("{res_str}");
        Ok(())
    }

    pub async fn run_and_get_hash(&self) -> Result<Hash, Error> {
        self.run_against_rpc_server(&self.wasm.read()?).await
    }

    async fn run_against_rpc_server(&self, contract: &[u8]) -> Result<Hash, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey =
            stellar_strkey::ed25519::PublicKey(key.verifying_key().to_bytes()).to_string();
        let account_details = client.get_account(&public_strkey).await?;
        let sequence: i64 = account_details.seq_num.into();

        let (tx_without_preflight, hash) =
            build_install_contract_code_tx(contract, sequence + 1, self.fee.fee, &key)?;

        // Currently internal errors are not returned if the contract code is expired
        if let (
            TransactionResult {
                result: TransactionResultResult::TxInternalError,
                ..
            },
            _,
            _,
        ) = client
            .prepare_and_send_transaction(
                &tx_without_preflight,
                &key,
                &[],
                &network.network_passphrase,
                None,
                None,
            )
            .await?
        {
            // Now just need to restore it and don't have to install again
            restore::Cmd {
                key: key::Args {
                    contract_id: None,
                    key: None,
                    key_xdr: None,
                    wasm: Some(self.wasm.wasm.clone()),
                    wasm_hash: None,
                    durability: super::Durability::Persistent,
                },
                config: self.config.clone(),
                fee: self.fee.clone(),
                ledgers_to_extend: None,
                ttl_ledger_only: true,
            }
            .run_against_rpc_server()
            .await?;
        }

        Ok(hash)
    }
}

pub(crate) fn build_install_contract_code_tx(
    source_code: &[u8],
    sequence: i64,
    fee: u32,
    key: &ed25519_dalek::SigningKey,
) -> Result<(Transaction, Hash), XdrError> {
    let hash = utils::contract_hash(source_code)?;

    let op = Operation {
        source_account: Some(MuxedAccount::Ed25519(Uint256(
            key.verifying_key().to_bytes(),
        ))),
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::UploadContractWasm(source_code.try_into()?),
            auth: VecM::default(),
        }),
    };

    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(key.verifying_key().to_bytes())),
        fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V0,
    };

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
            &utils::parse_secret_key("SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP")
                .unwrap(),
        );

        assert!(result.is_ok());
    }
}
