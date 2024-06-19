use olaf::{simplpedpop::AllMessage, SigningKeypair};
use serde_json::from_str;
use std::{
    fs::{self, File},
    io::Write,
};
use stellar_strkey::ed25519::PrivateKey;

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The folder that contains the files for the round 2 of the SimplPedPoP protocol
    #[arg(long)]
    pub files: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), super::Error> {
        let file_path: std::path::PathBuf = self.files.clone().into();

        let secret_key_string =
            fs::read_to_string(file_path.join("recipient_secret_key.json")).unwrap();

        let encoded_string: String = serde_json::from_str(&secret_key_string).unwrap();

        let sk = PrivateKey::from_string(&encoded_string).unwrap();

        let mut secret_key_bytes = [0; 32];
        secret_key_bytes.copy_from_slice(&sk.0);

        let mut keypair = SigningKeypair::from_secret_key(&secret_key_bytes);

        let all_messages_string = fs::read_to_string(file_path.join("all_messages.json")).unwrap();

        let all_messages_bytes: Vec<Vec<u8>> = from_str(&all_messages_string).unwrap();

        let all_messages: Vec<AllMessage> = all_messages_bytes
            .iter()
            .map(|all_message| AllMessage::from_bytes(all_message).unwrap())
            .collect();

        let simplpedpop = keypair.simplpedpop_recipient_all(&all_messages).unwrap();
        let output_round1 = simplpedpop.0;
        let output_json =
            serde_json::to_string_pretty(&output_round1.spp_output.to_bytes()).unwrap();

        let threshold_public_key_json = serde_json::to_string_pretty(
            &stellar_strkey::ed25519::PublicKey::from_payload(
                &output_round1.spp_output.threshold_public_key.0.to_bytes(),
            )
            .unwrap()
            .to_string(),
        )
        .unwrap();

        let mut output_file = File::create(file_path.join("spp_output.json")).unwrap();

        output_file.write_all(output_json.as_bytes()).unwrap();

        let mut threshold_public_key_file =
            File::create(file_path.join("threshold_public_key.json")).unwrap();

        threshold_public_key_file
            .write_all(threshold_public_key_json.as_bytes())
            .unwrap();

        let signing_share = simplpedpop.1;
        let signing_share_json =
            serde_json::to_string_pretty(&signing_share.to_bytes().to_vec()).unwrap();

        let mut signing_share_file = File::create(file_path.join("signing_share.json")).unwrap();

        signing_share_file
            .write_all(signing_share_json.as_bytes())
            .unwrap();

        Ok(())
    }
}
