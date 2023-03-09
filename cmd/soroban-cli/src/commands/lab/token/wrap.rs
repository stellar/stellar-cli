use clap::Parser;
use regex::Regex;
use sha2::{Digest, Sha256};
use soroban_env_host::{
    budget::Budget,
    storage::Storage,
    xdr::{
        AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, ContractId,
        CreateContractArgs, Error as XdrError, Hash, HashIdPreimage, HashIdPreimageFromAsset,
        HostFunction, InvokeHostFunctionOp, LedgerFootprint, LedgerKey::ContractData,
        LedgerKeyContractData, Memo, MuxedAccount, Operation, OperationBody, Preconditions,
        PublicKey, ScContractCode, ScObject, ScStatic::LedgerKeyContractCode, ScVal,
        SequenceNumber, Transaction, TransactionEnvelope, TransactionExt, Uint256, VecM, WriteXdr,
    },
    Host, HostError,
};
use std::{array::TryFromSliceError, fmt::Debug, num::ParseIntError, rc::Rc};

use crate::{
    commands::config,
    rpc::{Client, Error as SorobanRpcError},
    utils,
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

#[derive(Parser, Debug)]
pub struct Cmd {
    /// ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
    #[clap(long)]
    pub asset: String,

    #[clap(flatten)]
    pub config: config::Args,
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

    fn run_in_sandbox(&self, asset: &Asset) -> Result<String, Error> {
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

        let res = h.invoke_function(HostFunction::CreateContract(CreateContractArgs {
            contract_id: ContractId::Asset(asset.clone()),
            source: ScContractCode::Token,
        }))?;
        let res_str = utils::vec_to_hash(&res)?;

        state.update(&h);
        self.config.set_state(&mut state)?;
        Ok(res_str)
    }

    async fn run_against_rpc_server(&self, asset: Asset) -> Result<String, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url);
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey = stellar_strkey::ed25519::PublicKey(key.public.to_bytes()).to_string();
        // TODO: use symbols for the method names (both here and in serve)
        let account_details = client.get_account(&public_strkey).await?;
        // TODO: create a cmdline parameter for the fee instead of simply using the minimum fee
        let fee: u32 = 100;
        let sequence: i64 = account_details.seq_num.into();
        let network_passphrase = &network.network_passphrase;
        let contract_id = get_contract_id(&asset, network_passphrase)?;
        let tx = build_wrap_token_tx(
            &asset,
            &contract_id,
            sequence + 1,
            fee,
            network_passphrase,
            &key,
        )?;

        client.send_transaction(&tx).await?;

        Ok(hex::encode(&contract_id))
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
            key: ScVal::Object(Some(ScObject::Vec(
                vec![ScVal::Symbol("Metadata".try_into().unwrap())].try_into()?,
            ))),
        }),
    ];
    if asset != &Asset::Native {
        read_write.push(ContractData(LedgerKeyContractData {
            contract_id: contract_id.clone(),
            key: ScVal::Object(Some(ScObject::Vec(
                vec![ScVal::Symbol("Admin".try_into().unwrap())].try_into()?,
            ))),
        }));
    }

    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            function: HostFunction::CreateContract(CreateContractArgs {
                contract_id: ContractId::Asset(asset.clone()),
                source: ScContractCode::Token,
            }),
            footprint: LedgerFootprint {
                read_only: VecM::default(),
                read_write: read_write.try_into()?,
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

    Ok(utils::sign_transaction(key, &tx, network_passphrase)?)
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
