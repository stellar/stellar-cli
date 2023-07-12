use clap::{arg, command, Parser};
use soroban_env_host::{
    budget::Budget,
    storage::Storage,
    xdr::{
        Asset, ContractDataDurability, ContractEntryBodyType, ContractExecutable,
        ContractIdPreimage, CreateContractArgs, Error as XdrError, Hash, HostFunction,
        InvokeHostFunctionOp, LedgerKey::ContractData, LedgerKeyContractData, Memo, MuxedAccount,
        Operation, OperationBody, Preconditions, ScAddress, ScVal, SequenceNumber, Transaction,
        TransactionExt, Uint256, VecM,
    },
    Host, HostError,
};
use std::convert::Infallible;
use std::{array::TryFromSliceError, fmt::Debug, num::ParseIntError, rc::Rc};

use crate::{
    commands::config,
    rpc::{Client, Error as SorobanRpcError},
    utils::{contract_id_hash_from_asset, parsing::parse_asset},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    // TODO: the Display impl of host errors is pretty user-unfriendly
    //       (it just calls Debug). I think we can do better than that
    Host(#[from] HostError),
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
    #[error(transparent)]
    ParseAssetError(#[from] crate::utils::parsing::Error),
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

        let res = h.invoke_function(HostFunction::CreateContract(CreateContractArgs {
            contract_id_preimage: ContractIdPreimage::Asset(asset.clone()),
            executable: ContractExecutable::Token,
        }))?;

        let contract_id = vec_to_hash(&res)?;

        state.update(&h);
        self.config.set_state(&mut state)?;
        Ok(stellar_strkey::Contract(contract_id.0).to_string())
    }

    async fn run_against_rpc_server(&self, asset: Asset) -> Result<String, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey = stellar_strkey::ed25519::PublicKey(key.public.to_bytes()).to_string();
        // TODO: use symbols for the method names (both here and in serve)
        let account_details = client.get_account(&public_strkey).await?;
        let sequence: i64 = account_details.seq_num.into();
        let network_passphrase = &network.network_passphrase;
        let contract_id = contract_id_hash_from_asset(&asset, network_passphrase)?;
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

fn build_wrap_token_tx(
    asset: &Asset,
    contract_id: &Hash,
    sequence: i64,
    fee: u32,
    _network_passphrase: &str,
    key: &ed25519_dalek::Keypair,
) -> Result<Transaction, Error> {
    let contract = ScAddress::Contract(contract_id.clone());
    let mut read_write = vec![
        ContractData(LedgerKeyContractData {
            contract: contract.clone(),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
            body_type: ContractEntryBodyType::DataEntry,
        }),
        ContractData(LedgerKeyContractData {
            contract: contract.clone(),
            key: ScVal::Vec(Some(
                vec![ScVal::Symbol("Metadata".try_into().unwrap())].try_into()?,
            )),
            durability: ContractDataDurability::Persistent,
            body_type: ContractEntryBodyType::DataEntry,
        }),
    ];
    if asset != &Asset::Native {
        read_write.push(ContractData(LedgerKeyContractData {
            contract,
            key: ScVal::Vec(Some(
                vec![ScVal::Symbol("Admin".try_into().unwrap())].try_into()?,
            )),
            durability: ContractDataDurability::Persistent,
            body_type: ContractEntryBodyType::DataEntry,
        }));
    }

    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::CreateContract(CreateContractArgs {
                contract_id_preimage: ContractIdPreimage::Asset(asset.clone()),
                executable: ContractExecutable::Token,
            }),
            auth: VecM::default(),
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
