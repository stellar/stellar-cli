use olaf::SigningKeypair;
use rand::rngs::OsRng;
use serde_json::from_str;
use std::{
    fs::{self, File},
    io::Write,
};

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The folder that contains the files for the round 1 of the FROST protocol
    #[arg(long)]
    pub files: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), super::Error> {
        let file_path: std::path::PathBuf = self.files.clone().into();

        let signing_share_string =
            fs::read_to_string(file_path.join("signing_share.json")).unwrap();

        let signing_share_vec: Vec<u8> = from_str(&signing_share_string).unwrap();

        let mut signing_share_bytes = [0; 64];
        signing_share_bytes.copy_from_slice(&signing_share_vec);

        let signing_share = SigningKeypair::from_bytes(&signing_share_bytes).unwrap();

        let (signing_nonces, signing_commitments) = signing_share.commit(&mut OsRng);

        let signing_nonces_json =
            serde_json::to_string_pretty(&signing_nonces.to_bytes().to_vec()).unwrap();

        let mut signing_nonces_file = File::create(file_path.join("signing_nonces.json")).unwrap();

        signing_nonces_file
            .write_all(signing_nonces_json.as_bytes())
            .unwrap();

        let signing_commitments_vec = vec![signing_commitments.to_bytes().to_vec()];

        let signing_commitments_json =
            serde_json::to_string_pretty(&signing_commitments_vec).unwrap();

        let mut signing_commitments_file =
            File::create(file_path.join("signing_commitments.json")).unwrap();

        signing_commitments_file
            .write_all(signing_commitments_json.as_bytes())
            .unwrap();

        Ok(())
    }
}
