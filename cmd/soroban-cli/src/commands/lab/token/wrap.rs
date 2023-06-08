use clap::{arg, command, Parser};
use regex::Regex;
use sha2::{Digest, Sha256};
use soroban_env_host::{
    budget::Budget,
    storage::Storage,
    xdr::{
        AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, ContractId,
        CreateContractArgs, Error as XdrError, Hash, HashIdPreimage, HashIdPreimageFromAsset,
        HostFunction, HostFunctionArgs, InvokeHostFunctionOp, LedgerKey::ContractData,
        LedgerKeyContractData, Memo, MuxedAccount, Operation, OperationBody, Preconditions,
        PublicKey, ScContractExecutable, ScVal, SequenceNumber, Transaction, TransactionExt,
        Uint256, VecM, WriteXdr,
    },
    Host, HostError,
};
use std::convert::Infallible;
use std::{array::TryFromSliceError, fmt::Debug, num::ParseIntError, rc::Rc};

use crate::{
    commands::config,
    rpc::{Client, Error as SorobanRpcError},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot parse account id: {account_id}")]
    CannotParseAccountId { account_id: String },
    #[error("cannot parse asset: {asset}")]
    CannotParseAsset { asset: String },
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
    #[error(transparent)]
    Config(#[from] config::Error),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
    #[arg(long)]
    pub asset: String,

    #[command(flatten)]
    pub config: config::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        // Parse asset
        let asset = parse_asset(&self.asset)?;

        let res_str = if self.config.is_no_network() {
            self.run_in_sandbox(&asset)?
        } else {
            self.run_against_rpc_server(asset).await?
        };
        println!("{res_str}");
        Ok(())
    }

    pub fn run_in_sandbox(&self, asset: &Asset) -> Result<String, Error> {
        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let mut state = self.config.get_state()?;

        let snap = Rc::new(state.clone());
        let h = Host::with_storage_and_budget(
            Storage::with_recording_footprint(snap),
            Budget::default(),
        );

        let mut ledger_info = state.ledger_info();
        ledger_info.sequence_number += 1;
        ledger_info.timestamp += 5;
        h.set_ledger_info(ledger_info);

        let res = h.invoke_functions(vec![HostFunction {
            args: HostFunctionArgs::CreateContract(CreateContractArgs {
                contract_id: ContractId::Asset(asset.clone()),
                executable: ScContractExecutable::Token,
            }),
            auth: VecM::default(),
        }])?;

        let contract_id = vec_to_hash(&res[0])?;

        state.update(&h);
        self.config.set_state(&mut state)?;
        Ok(stellar_strkey::Contract(contract_id.0).to_string())
    }

    async fn run_against_rpc_server(&self, asset: Asset) -> Result<String, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey = stellar_strkey::ed25519::PublicKey(key.public.to_bytes()).to_string();
        // TODO: use symbols for the method names (both here and in serve)
        let account_details = client.get_account(&public_strkey).await?;
        let sequence: i64 = account_details.seq_num.into();
        let network_passphrase = &network.network_passphrase;
        let contract_id = get_contract_id(&asset, network_passphrase)?;
        let tx = build_wrap_token_tx(
            &asset,
            &contract_id,
            sequence + 1,
            self.fee.fee,
            network_passphrase,
            &key,
        )?;

        client
            .prepare_and_send_transaction(&tx, &key, network_passphrase, None)
            .await?;

        Ok(stellar_strkey::Contract(contract_id.0).to_string())
    }
}

/// # Errors
///
/// Might return an error
pub fn vec_to_hash(res: &ScVal) -> Result<Hash, XdrError> {
    if let ScVal::Bytes(res_hash) = &res {
        let mut hash_bytes: [u8; 32] = [0; 32];
        for (i, b) in res_hash.iter().enumerate() {
            hash_bytes[i] = *b;
        }
        Ok(Hash(hash_bytes))
    } else {
        Err(XdrError::Invalid)
    }
}

fn get_contract_id(asset: &Asset, network_passphrase: &str) -> Result<Hash, Error> {
    let network_id = Hash(
        Sha256::digest(network_passphrase.as_bytes())
            .try_into()
            .unwrap(),
    );
    let preimage = HashIdPreimage::ContractIdFromAsset(HashIdPreimageFromAsset {
        network_id,
        asset: asset.clone(),
    });
    let preimage_xdr = preimage.to_xdr()?;
    Ok(Hash(Sha256::digest(preimage_xdr).into()))
}

fn build_wrap_token_tx(
    asset: &Asset,
    contract_id: &Hash,
    sequence: i64,
    fee: u32,
    _network_passphrase: &str,
    key: &ed25519_dalek::Keypair,
) -> Result<Transaction, Error> {
    let mut read_write = vec![
        ContractData(LedgerKeyContractData {
            contract_id: contract_id.clone(),
            key: ScVal::LedgerKeyContractExecutable,
        }),
        ContractData(LedgerKeyContractData {
            contract_id: contract_id.clone(),
            key: ScVal::Vec(Some(
                vec![ScVal::Symbol("Metadata".try_into().unwrap())].try_into()?,
            )),
        }),
    ];
    if asset != &Asset::Native {
        read_write.push(ContractData(LedgerKeyContractData {
            contract_id: contract_id.clone(),
            key: ScVal::Vec(Some(
                vec![ScVal::Symbol("Admin".try_into().unwrap())].try_into()?,
            )),
        }));
    }

    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            functions: vec![HostFunction {
                args: HostFunctionArgs::CreateContract(CreateContractArgs {
                    contract_id: ContractId::Asset(asset.clone()),
                    executable: ScContractExecutable::Token,
                }),
                auth: VecM::default(),
            }]
            .try_into()?,
        }),
    };

    Ok(Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(key.public.to_bytes())),
        fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V0,
    })
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
    let pk_bytes = stellar_strkey::ed25519::PublicKey::from_string(str)
        .map_err(|_| Error::CannotParseAccountId {
            account_id: str.to_string(),
        })?
        .0;
    Ok(AccountId(PublicKey::PublicKeyTypeEd25519(pk_bytes.into())))
}
