use std::array::TryFromSliceError;
use std::fmt::Debug;
use std::num::ParseIntError;

use crate::rpc::{self, Client};
use crate::{commands::config, utils, wasm};
use clap::{command, Parser};
use soroban_env_host::xdr::{
    Error as XdrError, Hash, HostFunction, InvokeHostFunctionOp, Memo, MuxedAccount, Operation,
    OperationBody, Preconditions, SequenceNumber, Transaction, TransactionExt, Uint256, VecM,
};

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
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let res_str = hex::encode(self.run_and_get_hash().await?);
        println!("{res_str}");
        Ok(())
    }

    pub async fn run_and_get_hash(&self) -> Result<Hash, Error> {
        let contract = self.wasm.read()?;
        if self.config.is_no_network() {
            self.run_in_sandbox(contract)
        } else {
            self.run_against_rpc_server(contract).await
        }
    }

    pub fn run_in_sandbox(&self, contract: Vec<u8>) -> Result<Hash, Error> {
        let mut state = self.config.get_state()?;
        let wasm_hash = utils::add_contract_code_to_ledger_entries(
            &mut state.ledger_entries,
            contract,
            state.min_persistent_entry_expiration,
        )?;

        self.config.set_state(&mut state)?;

        Ok(wasm_hash)
    }

    async fn run_against_rpc_server(&self, contract: Vec<u8>) -> Result<Hash, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey = stellar_strkey::ed25519::PublicKey(key.public.to_bytes()).to_string();
        let account_details = client.get_account(&public_strkey).await?;
        let sequence: i64 = account_details.seq_num.into();

        let (tx_without_preflight, hash) =
            build_install_contract_code_tx(contract.clone(), sequence + 1, self.fee.fee, &key)?;

        client
            .prepare_and_send_transaction(
                &tx_without_preflight,
                &key,
                &network.network_passphrase,
                None,
            )
            .await?;

        Ok(hash)
    }
}

pub(crate) fn build_install_contract_code_tx(
    source_code: Vec<u8>,
    sequence: i64,
    fee: u32,
    key: &ed25519_dalek::Keypair,
) -> Result<(Transaction, Hash), XdrError> {
    let hash = utils::contract_hash(&source_code)?;

    let op = Operation {
        source_account: Some(MuxedAccount::Ed25519(Uint256(key.public.to_bytes()))),
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::UploadContractWasm(source_code.try_into()?),
            auth: VecM::default(),
        }),
    };

    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(key.public.to_bytes())),
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
            b"foo".to_vec(),
            300,
            1,
            &utils::parse_secret_key("SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP")
                .unwrap(),
        );

        assert!(result.is_ok());
    }
}
