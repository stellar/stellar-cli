use ed25519_dalek::VerifyingKey;
use olaf::{simplpedpop::AllMessage, SigningKeypair};
use std::{
    fs::{self, File},
    io::Write,
};
use stellar_strkey::ed25519::PrivateKey;

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The threshold for the SimplPedPoP protocol
    #[arg(long)]
    threshold: u16,
    /// The folder that contains the files for the round 1 of the SimplPedPoP protocol
    #[arg(long)]
    pub files: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), super::Error> {
        let file_path: std::path::PathBuf = self.files.clone().into();

        let secret_key_string =
            fs::read_to_string(file_path.join("contributor_secret_key.json")).unwrap();

        let encoded_string: String = serde_json::from_str(&secret_key_string).unwrap();

        let sk = PrivateKey::from_string(&encoded_string).unwrap();

        let mut secret_key_bytes = [0; 32];
        secret_key_bytes.copy_from_slice(&sk.0);

        let mut keypair = SigningKeypair::from_secret_key(&secret_key_bytes);

        let recipients_string = fs::read_to_string(file_path.join("recipients.json")).unwrap();

        let encoded_strings: Vec<String> = serde_json::from_str(&recipients_string).unwrap();

        let recipients: Vec<VerifyingKey> = encoded_strings
            .iter()
            .map(|encoded_string| {
                let pk = stellar_strkey::ed25519::PublicKey::from_string(encoded_string).unwrap();
                let mut recipient = [0; 32];
                recipient.copy_from_slice(&pk.0);
                VerifyingKey::from_bytes(&recipient).unwrap()
            })
            .collect();

        let all_message: AllMessage = keypair
            .simplpedpop_contribute_all(self.threshold, recipients)
            .unwrap();

        let all_message_bytes: Vec<u8> = all_message.to_bytes();
        let all_message_vec: Vec<Vec<u8>> = vec![all_message_bytes];

        let all_message_json = serde_json::to_string_pretty(&all_message_vec).unwrap();

        let mut all_message_file = File::create(file_path.join("all_messages.json")).unwrap();

        all_message_file
            .write_all(all_message_json.as_bytes())
            .unwrap();

        Ok(())
    }
}
