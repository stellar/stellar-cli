use std::array::TryFromSliceError;
use std::fmt::Debug;
use std::num::ParseIntError;

use clap::Parser;
use soroban_env_host::xdr::{
    Error as XdrError, Hash, HostFunction, InstallContractCodeArgs, InvokeHostFunctionOp,
    LedgerFootprint, LedgerKey::ContractCode, LedgerKeyContractCode, Memo, MuxedAccount, Operation,
    OperationBody, Preconditions, SequenceNumber, Transaction, TransactionEnvelope, TransactionExt,
    Uint256, VecM,
};
use soroban_env_host::HostError;

use crate::rpc::{self, Client};
use crate::{commands::config, utils, wasm};

#[derive(Parser, Debug)]
pub struct Cmd {
    #[clap(flatten)]
    pub wasm: wasm::Args,
    #[clap(flatten)]
    pub config: config::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Host(#[from] HostError),
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
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let res_str = self.run_and_get_hash().await?;
        println!("{res_str}");
        Ok(())
    }

    pub async fn run_and_get_hash(&self) -> Result<String, Error> {
        let contract = self.wasm.read()?;
        if self.config.is_no_network() {
            self.run_in_sandbox(contract)
        } else {
            self.run_against_rpc_server(contract).await
        }
    }

    fn run_in_sandbox(&self, contract: Vec<u8>) -> Result<String, Error> {
        let mut state = self.config.get_state()?;
        let wasm_hash =
            utils::add_contract_code_to_ledger_entries(&mut state.ledger_entries, contract)?;

        self.config.set_state(&mut state)?;

        Ok(hex::encode(wasm_hash))
    }

    async fn run_against_rpc_server(&self, contract: Vec<u8>) -> Result<String, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url);
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey = stellar_strkey::ed25519::PublicKey(key.public.to_bytes()).to_string();
        let account_details = client.get_account(&public_strkey).await?;
        // TODO: create a cmdline parameter for the fee instead of simply using the minimum fee
        let fee: u32 = 100;
        let sequence: i64 = account_details.seq_num.into();

        let (tx, hash) = build_install_contract_code_tx(
            contract,
            sequence + 1,
            fee,
            &network.network_passphrase,
            &key,
        )?;
        client.send_transaction(&tx).await?;

        Ok(hex::encode(hash.0))
    }
}

pub(crate) fn build_install_contract_code_tx(
    contract: Vec<u8>,
    sequence: i64,
    fee: u32,
    network_passphrase: &str,
    key: &ed25519_dalek::Keypair,
) -> Result<(TransactionEnvelope, Hash), XdrError> {
    let hash = utils::contract_hash(&contract)?;

    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            function: HostFunction::InstallContractCode(InstallContractCodeArgs {
                code: contract.try_into()?,
            }),
            footprint: LedgerFootprint {
                read_only: VecM::default(),
                read_write: vec![ContractCode(LedgerKeyContractCode { hash: hash.clone() })]
                    .try_into()?,
            },
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

    let envelope = utils::sign_transaction(key, &tx, network_passphrase)?;

    Ok((envelope, hash))
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
            "Public Global Stellar Network ; September 2015",
            &utils::parse_secret_key("SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP")
                .unwrap(),
        );

        assert!(result.is_ok());
    }
}
