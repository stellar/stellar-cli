use std::{fmt::Debug, fs, io};

use clap::Parser;
use ed25519_dalek;
use ed25519_dalek::Signer;
use hex::FromHexError;
use sha2::{Digest, Sha256, Sha512};
use soroban_env_host::xdr::LedgerKey::ContractData;
use soroban_env_host::xdr::ScStatic::LedgerKeyContractCode;
use soroban_env_host::xdr::{
    Hash, HashIdPreimageEd25519ContractId, HostFunction, InvokeHostFunctionOp, LedgerFootprint,
    LedgerKey, LedgerKeyContractData, Memo, Operation, OperationBody, Preconditions, ScObject,
    ScVal, Transaction, TransactionEnvelope, TransactionExt, TransactionV1Envelope, Uint256,
    WriteXdr,
};
use soroban_env_host::{xdr::Error as XdrError, HostError};
use stellar_strkey::{DecodeError, StrkeyPrivateKeyEd25519};

use crate::snapshot::{self, get_default_ledger_info};
use crate::utils;

#[derive(Parser, Debug)]
pub struct Cmd {
    #[clap(long = "id")]
    /// Contract ID to deploy to
    contract_id: String,
    /// WASM file to deploy
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Host(#[from] HostError),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
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
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let contract_id: [u8; 32] =
            utils::contract_id_from_str(&self.contract_id).map_err(|e| {
                Error::CannotParseContractId {
                    contract_id: self.contract_id.clone(),
                    error: e,
                }
            })?;
        let contract = fs::read(&self.wasm).map_err(|e| Error::CannotReadContractFile {
            filepath: self.wasm.clone(),
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

    fn build_create_contract_tx(
        contract: Vec<u8>,
        key: ed25519_dalek::Keypair,
    ) -> Result<TransactionEnvelope, Error> {
        // TODO: generate the salt
        // TODO: should the salt be provided by the end user?
        let salt = Sha256::digest(b"a1");

        let separator =
            b"create_contract_from_ed25519(contract: Vec<u8>, salt: u256, key: u256, sig: Vec<u8>)";
        let mut hasher = Sha256::new();
        hasher.update(separator);
        hasher.update(salt);
        hasher.update(contract);
        let hash = hasher.finalize();

        let sig = key.sign(&hash);

        let preimage = HashIdPreimageEd25519ContractId {
            ed25519: Uint256(key.secret.as_bytes().clone()),
            salt: Uint256(salt.into()),
        };
        let preimage_xdr = preimage.to_xdr()?;
        let contract_id = Sha256::digest(preimage_xdr);

        // TODO: clean up duplicated code and check whether the type conversions here make sense
        let contract_parameter = ScVal::Object(Some(ScObject::Bytes(contract.try_into()?)));
        let salt_parameter = ScVal::Object(Some(ScObject::Bytes(salt.into()?)));
        let public_key_parameter =
            ScVal::Object(Some(ScObject::Bytes(key.public.as_bytes().into()?)));
        let signature_parameter = ScVal::Object(Some(ScObject::Bytes(sig.to_bytes().into()?)));

        // TODO: reorder code properly
        let lk = LedgerKey::ContractData(LedgerKeyContractData {
            contract_id: Hash(contract_id.into()),
            key: ScVal::Static(LedgerKeyContractCode),
        });

        let op = Operation {
            source_account: None,
            body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                function: HostFunction::CreateContract,
                parameters:
                 // TODO: cast to VecM
                vec![
                    contract_parameter,
                    salt_parameter,
                    public_key_parameter,
                    signature_parameter,
                ],
                footprint: LedgerFootprint {
                    read_only: Default::default(),
                    // TODO: how to convert this to VecM?
                    read_write: vec![lk],
                },
            }),
        };

        // TODO: sign transaction
        let tx = Transaction {
            source_account: Default::default(),
            // TODO: should the user supply the fee?
            fee: 0,
            // TODO: get sequence number from RPC server
            seq_num: SequenceNumber(),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: Default::default(),
            ext: TransactionExt::V0,
        };
    }

    fn parse_private_key(strkey: String) -> Result<ed25519_dalek::Keypair, Error> {
        let seed = stellar_strkey::StrkeyPrivateKeyEd25519::from_string(&strkey)
            .map_err(|_| Error::CannotParsePrivateKey)?;
        // TODO: improve error?
        let secret_key = ed25519_dalek::SecretKey::from_bytes(&seed.0)
            .map_err(|_| Error::CannotParsePrivateKey)?;
        let public_key = (&secret_key).into();
        Ok(ed25519_dalek::Keypair {
            secret: secret_key,
            public: public_key,
        })
    }
}
