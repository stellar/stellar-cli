use std::{ffi::OsString, fmt::Display, fs, path::PathBuf};

use assert_cmd::{assert::Assert, Command};
use assert_fs::{prelude::PathChild, TempDir};
use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{Error as XdrError, Hash, InstallContractCodeArgs, WriteXdr};

pub struct Wasm<'a>(pub &'a str);

impl Wasm<'_> {
    pub fn path(&self) -> PathBuf {
        let mut path = PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR")
                .map_or_else(|_| "", |_| "../..")
                .to_string(),
        )
        .join("target/wasm32-unknown-unknown/test-wasms")
        .join(self.0);
        path.set_extension("wasm");
        assert!(path.is_file(), "File not found: {}. run 'make build-test-wasms' to generate .wasm files before running this test", path.display());
        std::env::current_dir().unwrap().join(path)
    }

    pub fn bytes(&self) -> Vec<u8> {
        fs::read(self.path()).unwrap()
    }

    pub fn hash(&self) -> Hash {
        contract_hash(&self.bytes()).unwrap()
    }
}

/// Default
pub struct Sandbox {
    pub temp_dir: TempDir,
}

impl Sandbox {
    pub fn new() -> Sandbox {
        Self {
            temp_dir: TempDir::new().expect("failed to create temp dir"),
        }
    }
    pub fn new_cmd(&self, name: &str) -> Command {
        let mut this = Command::cargo_bin("soroban").expect("failed to find local soroban binary");
        this.arg(name);
        this.current_dir(&self.temp_dir);
        this
    }

    pub fn dir(&self) -> &TempDir {
        &self.temp_dir
    }

    pub fn gen_test_identity(&self) {
        self.new_cmd("config")
            .arg("identity")
            .arg("generate")
            .arg("--default-seed")
            .arg("test")
            .assert()
            .success();
    }

    pub fn test_address(&self, hd_path: usize) -> String {
        self.new_cmd("config")
            .args("identity address test --hd-path".split(' '))
            .arg(format!("{hd_path}"))
            .assert()
            .stdout_as_str()
    }
}

pub fn temp_ledger_file() -> OsString {
    TempDir::new()
        .unwrap()
        .child("ledger.json")
        .as_os_str()
        .into()
}

pub trait AssertExt {
    fn stdout_as_str(&self) -> String;
}

impl AssertExt for Assert {
    fn stdout_as_str(&self) -> String {
        String::from_utf8(self.get_output().stdout.clone())
            .expect("failed to make str")
            .trim()
            .to_owned()
    }
}
pub trait CommandExt {
    fn json_arg<A>(&mut self, j: A) -> &mut Self
    where
        A: Display;
}

impl CommandExt for Command {
    fn json_arg<A>(&mut self, j: A) -> &mut Self
    where
        A: Display,
    {
        self.arg(OsString::from(j.to_string()))
    }
}

// TODO add a `lib.rs` so that this can be imported
pub fn contract_hash(contract: &[u8]) -> Result<Hash, XdrError> {
    let args_xdr = InstallContractCodeArgs {
        code: contract.try_into()?,
    }
    .to_xdr()?;
    Ok(Hash(Sha256::digest(args_xdr).into()))
}

pub const HELLO_WORLD: &Wasm = &Wasm("test_hello_world");
pub const CUSTOM_TYPES: &Wasm = &Wasm("test_custom_types");

#[allow(unused)]
pub fn temp_dir() -> TempDir {
    TempDir::new().unwrap()
}

#[derive(Clone)]
pub enum SecretKind {
    Seed,
    Key,
}

#[allow(clippy::needless_pass_by_value)]
pub fn add_identity(dir: &TempDir, name: &str, kind: SecretKind, data: &str) {
    let identity_dir = dir.join(".soroban").join("identities");
    fs::create_dir_all(&identity_dir).unwrap();
    let kind_str = match kind {
        SecretKind::Seed => "seed_phrase",
        SecretKind::Key => "secret_key",
    };
    let contents = format!("{kind_str} = \"{data}\"\n");
    fs::write(identity_dir.join(format!("{name}.toml")), contents).unwrap();
}

pub fn add_test_id(dir: &TempDir) -> String {
    let name = "test_id";
    add_identity(
        dir,
        name,
        SecretKind::Key,
        "SBGWSG6BTNCKCOB3DIFBGCVMUPQFYPA2G4O34RMTB343OYPXU5DJDVMN",
    );
    name.to_owned()
}

pub fn add_test_seed(dir: &TempDir) -> String {
    let name = "test_seed";
    add_identity(
        dir,
        name,
        SecretKind::Seed,
        "coral light army gather adapt blossom school alcohol coral light army giggle",
    );
    name.to_owned()
}

pub fn invoke(sandbox: &Sandbox, func: &str) -> Command {
    let mut s = sandbox.new_cmd("contract");
    s.arg("invoke")
        .arg("--id=1")
        .arg("--wasm")
        .arg(CUSTOM_TYPES.path())
        .arg("--")
        .arg(func);
    s
}

pub fn invoke_help(sandbox: &Sandbox) -> Command {
    let mut s = sandbox.new_cmd("contract");
    s.arg("invoke")
        .arg("--id=1")
        .arg("--wasm")
        .arg(CUSTOM_TYPES.path())
        .arg("--")
        .arg("--help");
    s
}

pub fn invoke_with_roundtrip<D>(func: &str, data: D)
where
    D: Display,
{
    invoke(&Sandbox::new(), func)
        .arg(&format!("--{func}"))
        .json_arg(&data)
        .assert()
        .success()
        .stdout(format!("{data}\n"));
}

pub const DEFAULT_SEED_PHRASE: &str =
    "coral light army gather adapt blossom school alcohol coral light army giggle";

pub const DEFAULT_PUB_KEY: &str = "GDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCR4W4";

pub const DEFAULT_PUB_KEY_1: &str = "GCKZUJVUNEFGD4HLFBUNVYM2QY2P5WQQZMGRA3DDL4HYVT5MW5KG3ODV";
