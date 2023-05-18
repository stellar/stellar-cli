use std::array::TryFromSliceError;
use std::fmt::Debug;
use std::num::ParseIntError;

use crate::rpc::{self, Client};
use crate::{commands::config, utils, wasm};
use clap::{command, Parser};
use soroban_env_host::xdr::{
    Error as XdrError, ExtensionPoint, Hash, HostFunction, HostFunctionArgs, InvokeHostFunctionOp,
    LedgerFootprint, Memo, MuxedAccount, Operation,
    OperationBody, Preconditions, SequenceNumber, SorobanResources, Transaction,
    TransactionEnvelope, TransactionExt, Uint256, UploadContractWasmArgs, VecM, ContractAuth,
    DiagnosticEvent, SorobanTransactionData, ReadXdr,
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
        let wasm_hash =
            utils::add_contract_code_to_ledger_entries(&mut state.ledger_entries, contract)?;

        self.config.set_state(&mut state)?;

        Ok(wasm_hash)
    }

    async fn run_against_rpc_server(&self, contract: Vec<u8>) -> Result<Hash, Error> {
        println!("contract/install run_against_rpc_server...");
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey = stellar_strkey::ed25519::PublicKey(key.public.to_bytes()).to_string();
        let account_details = client.get_account(&public_strkey).await?;
        let sequence: i64 = account_details.seq_num.into();

        let (tx_without_preflight, _hash) = build_install_contract_code_tx(
            contract.clone(),
            sequence + 1,
            self.fee.fee,
            &network.network_passphrase,
            &key,
            None,
            None,
        )?;

        println!("contract/install run_against_rpc_server ( before simulate )...");
        let simulation_response = client.simulate_transaction(&tx_without_preflight).await?;
        if simulation_response.results.len() != 1 {
            return Err(Error::UnexpectedSimulateTransactionResultSize {
                length: simulation_response.results.len(),
            });
        }
        println!("contract/install run_against_rpc_server ( after simulate )...");

        let result = &simulation_response.results[0];
        let transaction_data =
            SorobanTransactionData::from_xdr_base64(&simulation_response.transaction_data)?;
        let auth = result
            .auth
            .iter()
            .map(ContractAuth::from_xdr_base64)
            .collect::<Result<Vec<_>, _>>()?;
        let events = simulation_response
            .events
            .iter()
            .map(DiagnosticEvent::from_xdr_base64)
            .collect::<Result<Vec<_>, _>>()?;
        if !events.is_empty() {
            tracing::debug!(simulation_events=?events);
        }

        //log_events(&transaction_data.resources.footprint, &auth, &[], None);

        // update the fees of the actual transaction to meet the minimum resource fees.
        let mut final_fees = self.fee.fee;
        let classic_transaction_fees = crate::fee::Args::default().fee;
        if final_fees < classic_transaction_fees + simulation_response.min_resource_fee {
            final_fees = classic_transaction_fees + simulation_response.min_resource_fee;
        }

        let (tx, hash) = build_install_contract_code_tx(
            contract,
            sequence + 1,
            final_fees,
            &network.network_passphrase,
            &key,
            Some(transaction_data),
            Some(auth),
        )?;

        println!("contract/install run_against_rpc_server ( before sending )...");
        client.send_transaction(&tx).await?;

        Ok(hash)
    }
}

pub(crate) fn build_install_contract_code_tx(
    contract: Vec<u8>,
    sequence: i64,
    fee: u32,
    network_passphrase: &str,
    key: &ed25519_dalek::Keypair,
    transaction_data: Option<SorobanTransactionData>,
    auth: Option<Vec<ContractAuth>>,
) -> Result<(TransactionEnvelope, Hash), XdrError> {
    let hash = utils::contract_hash(&contract)?;

    let final_auth = auth.unwrap_or(Vec::default());

    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            functions: vec![HostFunction {
                args: HostFunctionArgs::UploadContractWasm(UploadContractWasmArgs {
                    code: contract.try_into()?,
                }),
                auth: final_auth.try_into()?,
            }]
            .try_into()?,
        }),
    };

    let final_transaction_data = transaction_data.unwrap_or(SorobanTransactionData {
        resources: SorobanResources {
            footprint: LedgerFootprint {
                read_only: VecM::default(),
                read_write: VecM::default(),
            },
            instructions: 0,
            read_bytes: 0,
            write_bytes: 0,
            extended_meta_data_size_bytes: 0,
        },
        refundable_fee: 0,
        ext: ExtensionPoint::V0,
    });

    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(key.public.to_bytes())),
        fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V1(final_transaction_data),
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
            None,
            None,
        );

        assert!(result.is_ok());
    }
}
