use std::array::TryFromSliceError;
use std::num::ParseIntError;
use std::{fmt::Debug, fs, io};

use clap::Parser;
use ed25519_dalek::Signer;
use hex::FromHexError;
use rand::Rng;
use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{
    AccountId, DecoratedSignature, Error as XdrError, Hash, HashIdPreimage,
    HashIdPreimageSourceAccountContractId, HostFunction, InvokeHostFunctionOp, LedgerFootprint,
    LedgerKey::ContractData, LedgerKeyContractData, Memo, MuxedAccount, Operation, OperationBody,
    Preconditions, PublicKey, ScObject, ScStatic::LedgerKeyContractCode, ScVal, SequenceNumber,
    Signature, SignatureHint, Transaction, TransactionEnvelope, TransactionExt,
    TransactionV1Envelope, Uint256, VecM, WriteXdr,
};
use soroban_env_host::HostError;

use crate::rpc::{self, Client};
use crate::snapshot::{self, get_default_ledger_info};
use crate::utils;

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file to deploy
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
    #[clap(
        long = "id",
        required_unless_present = "rpc-server-url",
        conflicts_with = "rpc-server-url"
    )]
    // TODO: Should we get rid of the contract_id parameter
    //       and just obtain it from the key/source like we do
    //       when running against an rpc server?
    /// Contract ID to deploy to (if using the sandbox)
    contract_id: Option<String>,
    /// File to persist ledger state (if using the sandbox)
    #[clap(
        long,
        parse(from_os_str),
        default_value = ".soroban/ledger.json",
        conflicts_with = "rpc-server-url"
    )]
    ledger_file: std::path::PathBuf,

    /// RPC server endpoint
    #[clap(
        long,
        required_unless_present = "contract-id",
        conflicts_with = "contract-id",
        requires = "private-strkey",
        requires = "network-passphrase"
    )]
    rpc_server_url: Option<String>,
    /// Private key to sign the transaction sent to the rpc server
    #[clap(long = "private-strkey", env)]
    private_strkey: Option<String>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[clap(long = "network-passphrase")]
    network_passphrase: Option<String>,
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
    #[error("reading file {filepath}: {error}")]
    CannotReadLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("reading file {filepath}: {error}")]
    CannotReadContractFile {
        filepath: std::path::PathBuf,
        error: io::Error,
    },
    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: FromHexError,
    },
    #[error("cannot parse private key")]
    CannotParsePrivateKey,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        let contract = fs::read(&self.wasm).map_err(|e| Error::CannotReadContractFile {
            filepath: self.wasm.clone(),
            error: e,
        })?;

        if self.rpc_server_url.is_some() {
            return self.run_against_rpc_server(contract).await;
        }

        self.run_in_sandbox(contract)
    }

    fn run_in_sandbox(&self, contract: Vec<u8>) -> Result<(), Error> {
        let contract_id: [u8; 32] = utils::contract_id_from_str(self.contract_id.as_ref().unwrap())
            .map_err(|e| Error::CannotParseContractId {
                contract_id: self.contract_id.as_ref().unwrap().clone(),
                error: e,
            })?;

        let mut state =
            snapshot::read(&self.ledger_file).map_err(|e| Error::CannotReadLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            })?;
        utils::add_contract_to_ledger_entries(&mut state.1, contract_id, contract)?;

        snapshot::commit(state.1, get_default_ledger_info(), [], &self.ledger_file).map_err(
            |e| Error::CannotCommitLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            },
        )?;
        Ok(())
    }

    async fn run_against_rpc_server(&self, contract: Vec<u8>) -> Result<(), Error> {
        let client = Client::new(self.rpc_server_url.as_ref().unwrap());
        let key = utils::parse_private_key(self.private_strkey.as_ref().unwrap())
            .map_err(|_| Error::CannotParsePrivateKey)?;

        // Get the account sequence number
        let public_strkey =
            stellar_strkey::StrkeyPublicKeyEd25519(key.public.to_bytes()).to_string();
        // TODO: use symbols for the method names (both here and in serve)
        let account_details = client.get_account(&public_strkey).await?;
        // TODO: create a cmdline parameter for the fee instead of simply using the minimum fee
        let fee: u32 = 100;
        let sequence = account_details.sequence.parse::<i64>()?;
        let (tx, contract_id) = build_create_contract_tx(
            contract,
            sequence,
            fee,
            self.network_passphrase.as_ref().unwrap(),
            &key,
        )?;

        println!("Contract ID: {}", hex::encode(contract_id.0));

        client.send_transaction(&tx).await?;

        Ok(())
    }
}

fn build_create_contract_tx(
    contract: Vec<u8>,
    sequence: i64,
    fee: u32,
    network_passphrase: &str,
    key: &ed25519_dalek::Keypair,
) -> Result<(TransactionEnvelope, Hash), Error> {
    let salt = rand::thread_rng().gen::<[u8; 32]>();

    let preimage =
        HashIdPreimage::ContractIdFromSourceAccount(HashIdPreimageSourceAccountContractId {
            source_account: AccountId(PublicKey::PublicKeyTypeEd25519(
                key.public.to_bytes().into(),
            )),
            salt: Uint256(salt),
        });
    let preimage_xdr = preimage.to_xdr()?;
    let contract_id = Sha256::digest(preimage_xdr);

    let contract_parameter = ScVal::Object(Some(ScObject::Bytes(contract.try_into()?)));
    let salt_parameter = ScVal::Object(Some(ScObject::Bytes(salt.try_into()?)));

    let lk = ContractData(LedgerKeyContractData {
        contract_id: Hash(contract_id.into()),
        key: ScVal::Static(LedgerKeyContractCode),
    });

    let parameters: VecM<ScVal, 256_000> = vec![contract_parameter, salt_parameter].try_into()?;

    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            function: HostFunction::CreateContractWithSourceAccount,
            parameters: parameters.into(),
            footprint: LedgerFootprint {
                read_only: VecM::default(),
                read_write: vec![lk].try_into()?,
            },
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

    // sign the transaction
    let tx_hash = utils::transaction_hash(&tx, network_passphrase)?;
    let tx_signature = key.sign(&tx_hash);

    let decorated_signature = DecoratedSignature {
        hint: SignatureHint(key.public.to_bytes()[28..].try_into()?),
        signature: Signature(tx_signature.to_bytes().try_into()?),
    };

    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: vec![decorated_signature].try_into()?,
    });

    Ok((envelope, Hash(contract_id.into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_create_contract() {
        let result = build_create_contract_tx(
            b"foo".to_vec(),
            300,
            1,
            "Public Global Stellar Network ; September 2015",
            &utils::parse_private_key("SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP")
                .unwrap(),
        );

        assert!(result.is_ok());
    }
}
