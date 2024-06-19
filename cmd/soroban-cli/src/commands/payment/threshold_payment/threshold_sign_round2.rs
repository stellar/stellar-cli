use crate::{commands::config, utils::parsing::parse_asset};
use olaf::{
    frost::{SigningCommitments, SigningNonces},
    simplpedpop::SPPOutput,
    SigningKeypair,
};
use serde_json::from_str;
use sha2::{Digest, Sha256};
use soroban_env_host::xdr::Hash;
use soroban_rpc::Client;
use soroban_sdk::xdr::{
    Limits, Memo, MuxedAccount, Operation, OperationBody, PaymentOp, Preconditions, SequenceNumber,
    Transaction, TransactionExt, TransactionSignaturePayload,
    TransactionSignaturePayloadTaggedTransaction, Uint256, WriteXdr,
};
use std::{
    fs::{self, File},
    io::Write,
};

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[arg(long)]
    pub destination: String,
    #[arg(long)]
    pub amount: i64,
    #[arg(long)]
    pub asset: String,
    #[command(flatten)]
    pub config: config::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
    /// The folder that contains the files for the round 2 of the FROST protocol
    #[arg(long)]
    pub files: String,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), super::Error> {
        let file_path: std::path::PathBuf = self.files.clone().into();

        /*let mut bytes = [0; 32];
        thread_rng().fill(&mut bytes);
        let account = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(bytes)));
        println!("{account}");*/

        let signing_commitments_string =
            fs::read_to_string(file_path.join("signing_commitments.json")).unwrap();

        let signing_commitments_bytes: Vec<Vec<u8>> =
            from_str(&signing_commitments_string).unwrap();

        let signing_commitments: Vec<SigningCommitments> = signing_commitments_bytes
            .iter()
            .map(|signing_commitments| SigningCommitments::from_bytes(signing_commitments).unwrap())
            .collect();

        let signing_nonces_string =
            fs::read_to_string(file_path.join("signing_nonces.json")).unwrap();

        let signing_nonces_bytes: Vec<u8> = from_str(&signing_nonces_string).unwrap();
        let signing_nonces = SigningNonces::from_bytes(&signing_nonces_bytes).unwrap();

        let signing_share_string =
            fs::read_to_string(file_path.join("signing_share.json")).unwrap();

        let signing_share_vec: Vec<u8> = from_str(&signing_share_string).unwrap();

        let mut signing_share_bytes = [0; 64];
        signing_share_bytes.copy_from_slice(&signing_share_vec);

        let signing_share = SigningKeypair::from_bytes(&signing_share_bytes).unwrap();

        let output_string = fs::read_to_string(file_path.join("spp_output.json")).unwrap();

        let output_bytes: Vec<u8> = from_str(&output_string).unwrap();
        let spp_output = SPPOutput::from_bytes(&output_bytes).unwrap();

        let config = &self.config;

        let network = config.get_network().unwrap();
        let client = Client::new(&network.rpc_url).unwrap();

        let pk = stellar_strkey::ed25519::PublicKey::from_string(&config.source_account).unwrap();

        let account_details = client.get_account(&pk.to_string()).await.unwrap();
        let sequence: i64 = account_details.seq_num.into();
        let network_passphrase = &network.network_passphrase;

        let asset = parse_asset(&self.asset).unwrap();
        let destination =
            stellar_strkey::ed25519::PublicKey::from_string(&self.destination).unwrap();

        let op = Operation {
            source_account: None,
            body: OperationBody::Payment(PaymentOp {
                destination: MuxedAccount::Ed25519(Uint256(destination.0)),
                asset,
                amount: self.amount,
            }),
        };

        let tx = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(pk.0)),
            fee: self.fee.fee,
            seq_num: SequenceNumber(sequence + 1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![op].try_into().unwrap(),
            ext: TransactionExt::V0,
        };

        let signature_payload = TransactionSignaturePayload {
            network_id: Hash(Sha256::digest(network_passphrase).into()),
            tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone()),
        };

        let hash: [u8; 32] =
            Sha256::digest(signature_payload.to_xdr(Limits::none()).unwrap()).into();

        let signing_package = signing_share
            .sign(&hash, &spp_output, &signing_commitments, &signing_nonces)
            .unwrap();

        let signing_packages_vec = vec![signing_package.to_bytes()];

        let signing_package_json = serde_json::to_string_pretty(&signing_packages_vec).unwrap();

        let mut signing_package_file =
            File::create(file_path.join("signing_packages.json")).unwrap();

        signing_package_file
            .write_all(signing_package_json.as_bytes())
            .unwrap();

        let tx_encoded = tx.to_xdr_base64(Limits::none()).unwrap();

        let tx_encoded_json = serde_json::to_string_pretty(&tx_encoded).unwrap();

        let mut tx_encoded_file = File::create(file_path.join("tx_encoded.json")).unwrap();

        tx_encoded_file
            .write_all(tx_encoded_json.as_bytes())
            .unwrap();

        Ok(())
    }
}
