use crate::commands::config;
use olaf::frost::{aggregate, SigningPackage};
use serde_json::from_str;
use soroban_rpc::Client;
use soroban_sdk::xdr::{
    DecoratedSignature, Signature, SignatureHint, TransactionEnvelope, TransactionV1Envelope,
};
use soroban_sdk::xdr::{Limited, Limits, ReadXdr, Transaction};
use std::fs;

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The folder that contains the files for the aggregate round of the FROST protocol
    #[arg(long)]
    pub files: String,
    #[command(flatten)]
    pub config: config::Args,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), super::Error> {
        let file_path: std::path::PathBuf = self.files.clone().into();

        let signing_packages_string =
            fs::read_to_string(file_path.join("signing_packages.json")).unwrap();

        let signing_packages_bytes: Vec<Vec<u8>> = from_str(&signing_packages_string).unwrap();

        let signing_packages: Vec<SigningPackage> = signing_packages_bytes
            .iter()
            .map(|signing_commitments| SigningPackage::from_bytes(signing_commitments).unwrap())
            .collect();

        let tx_signature = aggregate(&signing_packages).unwrap();

        let config = &self.config;
        let network = config.get_network().unwrap();
        let client = Client::new(&network.rpc_url).unwrap();
        let pk = stellar_strkey::ed25519::PublicKey::from_string(&config.source_account).unwrap();

        let decorated_signature = DecoratedSignature {
            hint: SignatureHint(pk.0[28..].try_into().unwrap()),
            signature: Signature(tx_signature.to_bytes().try_into().unwrap()),
        };

        let tx_encoded_string = fs::read_to_string(file_path.join("tx_encoded.json")).unwrap();

        let tx_encoded_bytes: String = from_str(&tx_encoded_string).unwrap();

        let mut read = Limited::new(tx_encoded_bytes.as_bytes(), Limits::none());
        let tx_decoded = Transaction::read_xdr_base64(&mut read).unwrap();

        let tx = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: tx_decoded.clone(),
            signatures: vec![decorated_signature].try_into().unwrap(),
        });

        client.send_transaction(&tx).await.unwrap();

        Ok(())
    }
}
