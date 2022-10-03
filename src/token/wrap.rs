use clap::Parser;
use ed25519_dalek::Signer;
use regex::Regex;
use sha2::{Digest, Sha256};
use soroban_env_host::{
    budget::Budget,
    storage::Storage,
    xdr::{
        AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, DecoratedSignature,
        Error as XdrError, Hash, HashIdPreimage, HostFunction, InvokeHostFunctionOp,
        LedgerFootprint, LedgerKey::ContractData, LedgerKeyContractData, Memo, MuxedAccount,
        Operation, OperationBody, Preconditions, PublicKey, ScHostStorageErrorCode, ScObject,
        ScStatic::LedgerKeyContractCode, ScStatus, ScVal, SequenceNumber, Signature, SignatureHint,
        Transaction, TransactionEnvelope, TransactionExt, TransactionV1Envelope, Uint256, VecM,
        WriteXdr,
    },
    Host, HostError,
};
use std::{array::TryFromSliceError, fmt::Debug, num::ParseIntError, rc::Rc};
use stellar_strkey::StrkeyPublicKeyEd25519;

use crate::{
    rpc::{Client, Error as SorobanRpcError},
    snapshot, utils,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot parse account id: {account_id}")]
    CannotParseAccountId { account_id: String },
    #[error("cannot parse asset: {asset}")]
    CannotParseAsset { asset: String },
    #[error("cannot parse private key")]
    CannotParsePrivateKey,
    #[error("reading file {filepath}: {error}")]
    CannotReadLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error(transparent)]
    // TODO: the Display impl of host errors is pretty user-unfriendly
    //       (it just calls Debug). I think we can do better than that
    Host(#[from] HostError),
    #[error("invalid asset code: {asset}")]
    InvalidAssetCode { asset: String },
    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error(transparent)]
    Client(#[from] SorobanRpcError),
    #[error("internal conversion error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
}

#[derive(Parser, Debug)]
pub struct Cmd {
    /// ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
    #[clap(long)]
    asset: String,

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
        conflicts_with = "ledger-file",
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

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        // Parse asset
        let asset = parse_asset(&self.asset)?;

        let res_str = if self.rpc_server_url.is_some() {
            self.run_against_rpc_server(asset).await?
        } else {
            self.run_in_sandbox(&asset)?
        };
        println!("{}", res_str);
        Ok(())
    }

    fn run_in_sandbox(&self, asset: &Asset) -> Result<String, Error> {
        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let state = snapshot::read(&self.ledger_file).map_err(|e| Error::CannotReadLedgerFile {
            filepath: self.ledger_file.clone(),
            error: e,
        })?;

        let snap = Rc::new(snapshot::Snap {
            ledger_entries: state.1.clone(),
        });
        let h = Host::with_storage_and_budget(
            Storage::with_recording_footprint(snap),
            Budget::default(),
        );

        let mut ledger_info = state.0.clone();
        ledger_info.sequence_number += 1;
        ledger_info.timestamp += 5;
        h.set_ledger_info(ledger_info.clone());

        let mut buf: Vec<u8> = vec![];
        asset.write_xdr(&mut buf)?;
        let parameters: VecM<ScVal, 256_000> =
            vec![ScVal::Object(Some(ScObject::Bytes(buf.try_into()?)))].try_into()?;

        let res = h.invoke_function(
            HostFunction::CreateTokenContractWithAsset,
            parameters.into(),
        )?;
        let res_str = utils::vec_to_hash(&res)?;

        let (storage, _, _) = h.try_finish().map_err(|_h| {
            HostError::from(ScStatus::HostStorageError(
                ScHostStorageErrorCode::UnknownError,
            ))
        })?;

        snapshot::commit(state.1, ledger_info, &storage.map, &self.ledger_file).map_err(|e| {
            Error::CannotCommitLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            }
        })?;
        Ok(res_str)
    }

    async fn run_against_rpc_server(&self, asset: Asset) -> Result<String, Error> {
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
        let contract_id = get_contract_id(&asset)?;
        let tx = build_wrap_token_tx(
            &asset,
            &contract_id,
            sequence,
            fee,
            self.network_passphrase.as_ref().unwrap(),
            &key,
        )?;

        client.send_transaction(&tx).await?;

        Ok(hex::encode(&contract_id))
    }
}

fn get_contract_id(asset: &Asset) -> Result<Hash, Error> {
    let preimage = HashIdPreimage::ContractIdFromAsset(asset.clone());
    let preimage_xdr = preimage.to_xdr()?;
    Ok(Hash(Sha256::digest(preimage_xdr).into()))
}

fn build_wrap_token_tx(
    asset: &Asset,
    contract_id: &Hash,
    sequence: i64,
    fee: u32,
    network_passphrase: &str,
    key: &ed25519_dalek::Keypair,
) -> Result<TransactionEnvelope, Error> {
    let mut read_write = vec![
        ContractData(LedgerKeyContractData {
            contract_id: contract_id.clone(),
            key: ScVal::Static(LedgerKeyContractCode),
        }),
        ContractData(LedgerKeyContractData {
            contract_id: contract_id.clone(),
            key: ScVal::Symbol("Metadata".try_into().unwrap()),
        }),
    ];
    if asset != &Asset::Native {
        read_write.push(ContractData(LedgerKeyContractData {
            contract_id: contract_id.clone(),
            key: ScVal::Symbol("Admin".try_into().unwrap()),
        }));
    }

    let mut buf: Vec<u8> = vec![];
    asset.write_xdr(&mut buf)?;
    let parameters: VecM<ScVal, 256_000> =
        vec![ScVal::Object(Some(ScObject::Bytes(buf.try_into()?)))].try_into()?;

    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            function: HostFunction::CreateTokenContractWithAsset,
            parameters: parameters.into(),
            footprint: LedgerFootprint {
                read_only: VecM::default(),
                read_write: read_write.try_into()?,
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

    Ok(envelope)
}

fn parse_asset(str: &str) -> Result<Asset, Error> {
    if str == "native" {
        return Ok(Asset::Native);
    }
    let split: Vec<&str> = str.splitn(2, ':').collect();
    if split.len() != 2 {
        return Err(Error::CannotParseAsset {
            asset: str.to_string(),
        });
    }
    let code = split[0];
    let issuer = split[1];
    let re = Regex::new("^[[:alnum:]]{1,12}$").unwrap();
    if !re.is_match(code) {
        return Err(Error::InvalidAssetCode {
            asset: str.to_string(),
        });
    }
    if code.len() <= 4 {
        let mut asset_code: [u8; 4] = [0; 4];
        for (i, b) in code.as_bytes().iter().enumerate() {
            asset_code[i] = *b;
        }
        Ok(Asset::CreditAlphanum4(AlphaNum4 {
            asset_code: AssetCode4(asset_code),
            issuer: parse_account_id(issuer)?,
        }))
    } else {
        let mut asset_code: [u8; 12] = [0; 12];
        for (i, b) in code.as_bytes().iter().enumerate() {
            asset_code[i] = *b;
        }
        Ok(Asset::CreditAlphanum12(AlphaNum12 {
            asset_code: AssetCode12(asset_code),
            issuer: parse_account_id(issuer)?,
        }))
    }
}

fn parse_account_id(str: &str) -> Result<AccountId, Error> {
    let pk_bytes = StrkeyPublicKeyEd25519::from_string(str)
        .map_err(|_| Error::CannotParseAccountId {
            account_id: str.to_string(),
        })?
        .0;
    Ok(AccountId(PublicKey::PublicKeyTypeEd25519(pk_bytes.into())))
}
