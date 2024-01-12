use core::fmt;
use std::fs::read_to_string;
use std::path::Path;
use std::{env, fs, io};

use clap::Parser;
use std::num::NonZeroU32;
use std::sync::atomic::AtomicBool;
use toml_edit::{Document, Formatted, InlineTable, TomlError, Value};

#[derive(Clone, Debug, PartialEq, clap::ValueEnum)]
pub enum ExampleContract {
    Account,
    Alloc,
    AtomicMultiswap,
    AtomicSwap,
    Auth,
    CrossContract,
    CustomTypes,
    DeepContractAuth,
    Deployer,
    Errors,
    Events,
    Fuzzing,
    HelloWorld,
    Increment,
    LiquidityPool,
    Logging,
    SimpleAccount,
    SingleOffer,
    Timelock,
    Token,
    UpgradeableContract,
    None,
}

impl fmt::Display for ExampleContract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExampleContract::Account => write!(f, "account"),
            ExampleContract::Alloc => write!(f, "alloc"),
            ExampleContract::AtomicMultiswap => write!(f, "atomic_multiswap"),
            ExampleContract::AtomicSwap => write!(f, "atomic_swap"),
            ExampleContract::Auth => write!(f, "auth"),
            ExampleContract::CrossContract => write!(f, "cross_contract"),
            ExampleContract::CustomTypes => write!(f, "custom_types"),
            ExampleContract::DeepContractAuth => write!(f, "deep_contract_auth"),
            ExampleContract::Deployer => write!(f, "deployer"),
            ExampleContract::Errors => write!(f, "errors"),
            ExampleContract::Events => write!(f, "events"),
            ExampleContract::Fuzzing => write!(f, "fuzzing"),
            ExampleContract::HelloWorld => write!(f, "hello_world"),
            ExampleContract::Increment => write!(f, "increment"),
            ExampleContract::LiquidityPool => write!(f, "liquidity_pool"),
            ExampleContract::Logging => write!(f, "logging"),
            ExampleContract::SimpleAccount => write!(f, "simple_account"),
            ExampleContract::SingleOffer => write!(f, "single_offer"),
            ExampleContract::Timelock => write!(f, "timelock"),
            ExampleContract::Token => write!(f, "token"),
            ExampleContract::UpgradeableContract => write!(f, "upgradeable_contract"),
            ExampleContract::None => write!(f, "none"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    pub project_path: String,

    /// optional flag to specify soroban example contracts to include
    #[arg(short, long, num_args = 1..=20, default_value = "none")]
    pub with_example: Vec<ExampleContract>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Io error: {0}")]
    CreateDirError(#[from] io::Error),

    // the gix::clone::Error is too large to include in the error enum as is, so we wrap it in a Box
    #[error("Failed to clone the template repository")]
    CloneError(#[from] Box<gix::clone::Error>),

    // the gix::clone::fetch::Error is too large to include in the error enum as is, so we wrap it in a Box
    #[error("Failed to fetch the template repository: {0}")]
    FetchError(#[from] Box<gix::clone::fetch::Error>),

    #[error("Failed to checkout the template repository: {0}")]
    CheckoutError(#[from] gix::clone::checkout::main_worktree::Error),

    #[error("Failed to parse Cargo.toml: {0}")]
    TomlParseError(#[from] TomlError),
}

const SOROBAN_EXAMPLES_URL: &str = "https://github.com/stellar/soroban-examples.git";

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) -> Result<(), Error> {
        println!("ℹ️  Initializing project at {}", self.project_path);
        let project_path = Path::new(&self.project_path);

        init(project_path, &self.with_example)?;

        Ok(())
    }
}

fn init(project_path: &Path, with_examples: &[ExampleContract]) -> Result<(), Error> {
    let cli_cmd_root = env!("CARGO_MANIFEST_DIR");
    let template_dir_path = Path::new(cli_cmd_root)
        .join("src")
        .join("utils")
        .join("contract-init-template");

    std::fs::create_dir_all(project_path)?;
    copy_contents(template_dir_path.as_path(), project_path)?;

    // if there are with-contract flags, include the example contracts
    if include_example_contracts(with_examples) {
        // create an examples temp dir in the temp dir
        let examples_dir = tempfile::tempdir()?;

        // clone the soroban-examples repo into temp dir
        clone_repo(SOROBAN_EXAMPLES_URL, examples_dir.path())?;

        // copy the example contracts into the project
        copy_example_contracts(examples_dir.path(), project_path, with_examples)?;
    }

    Ok(())
}

fn copy_contents(from: &Path, to: &Path) -> Result<(), Error> {
    let contents_to_exclude_from_copy = [
        ".git",
        ".github",
        "Makefile",
        "Cargo.lock",
        ".vscode",
        "target",
    ];
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let entry_name = entry.file_name().to_string_lossy().to_string();
        let new_path = to.join(&entry_name);

        if contents_to_exclude_from_copy.contains(&entry_name.as_str()) {
            continue;
        }

        if path.is_dir() {
            std::fs::create_dir_all(&new_path)?;
            copy_contents(&path, &new_path)?;
        } else {
            if file_exists(&new_path.to_string_lossy()) {
                println!(
                    "ℹ️  Skipped creating {} as it already exists",
                    &new_path.to_string_lossy()
                );
                continue;
            }

            println!("➕  Writing {}", &new_path.to_string_lossy());
            std::fs::copy(&path, &new_path)?;
        }
    }

    Ok(())
}

fn file_exists(file_path: &str) -> bool {
    if let Ok(metadata) = fs::metadata(file_path) {
        metadata.is_file()
    } else {
        false
    }
}

fn include_example_contracts(contracts: &[ExampleContract]) -> bool {
    !(contracts.len() == 1 && contracts[0] == ExampleContract::None)
}

fn clone_repo(from_url: &str, to_path: &Path) -> Result<(), Error> {
    let mut fetch = gix::clone::PrepareFetch::new(
        from_url,
        to_path,
        gix::create::Kind::WithWorktree,
        gix::create::Options {
            destination_must_be_empty: false,
            fs_capabilities: None,
        },
        gix::open::Options::isolated(),
    )
    .map_err(Box::new)?
    .with_shallow(gix::remote::fetch::Shallow::DepthAtRemote(
        NonZeroU32::new(1).unwrap(),
    ));

    let (mut prepare, _outcome) = fetch
        .fetch_then_checkout(gix::progress::Discard, &AtomicBool::new(false))
        .map_err(Box::new)?;

    let (_repo, _outcome) =
        prepare.main_worktree(gix::progress::Discard, &AtomicBool::new(false))?;

    Ok(())
}

fn copy_example_contracts(
    from: &Path,
    to: &Path,
    contracts: &[ExampleContract],
) -> Result<(), Error> {
    let project_contracts_path = to.join("contracts");
    for contract in contracts {
        println!("ℹ️  Initializing example contract: {}", contract);
        let contract_as_string = contract.to_string();
        let contract_path = Path::new(&contract_as_string);
        let from_contract_path = from.join(contract_path);
        let to_contract_path = project_contracts_path.join(contract_path);
        std::fs::create_dir_all(&to_contract_path)?;

        copy_contents(&from_contract_path, &to_contract_path)?;
        edit_contract_cargo_file(&to_contract_path)?;
    }

    Ok(())
}

fn edit_contract_cargo_file(contract_path: &Path) -> Result<(), Error> {
    let cargo_path = contract_path.join("Cargo.toml");
    let cargo_toml_str = read_to_string(&cargo_path)?;
    let mut doc = cargo_toml_str.parse::<Document>().unwrap();

    let mut workspace_table = InlineTable::new();
    workspace_table.insert("workspace", Value::Boolean(Formatted::new(true)));

    doc["dependencies"]["soroban-sdk"] =
        toml_edit::Item::Value(Value::InlineTable(workspace_table.clone()));
    doc["dev_dependencies"]["soroban-sdk"] =
        toml_edit::Item::Value(Value::InlineTable(workspace_table));

    doc.remove("profile");

    std::fs::write(&cargo_path, doc.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;

    use super::*;

    #[test]
    fn test_init() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_examples = vec![ExampleContract::None];
        init(project_dir.as_path(), &with_examples).unwrap();

        assert!(project_dir.as_path().join("README.md").exists());
        assert!(project_dir.as_path().join("contracts").exists());
        assert!(project_dir.as_path().join("Cargo.toml").exists());

        // check that it does not include certain template files and directories
        assert!(!project_dir.as_path().join(".git").exists());
        assert!(!project_dir.as_path().join(".github").exists());
        assert!(!project_dir.as_path().join("Cargo.lock").exists());
        assert!(!project_dir.as_path().join(".vscode").exists());

        temp_dir.close().unwrap()
    }

    #[test]
    fn test_init_including_example_contract() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_examples = vec![ExampleContract::Alloc];
        init(project_dir.as_path(), &with_examples).unwrap();

        assert!(project_dir.as_path().join("README.md").exists());
        assert!(project_dir
            .as_path()
            .join("contracts")
            .join("alloc")
            .exists());

        // check that it does not include certain template files and directories
        assert!(!project_dir.as_path().join(".git").exists());
        assert!(!project_dir.as_path().join(".github").exists());
        assert!(!project_dir.as_path().join("Cargo.lock").exists());
        assert!(!project_dir.as_path().join(".vscode").exists());

        // check that it does not include certain contract files
        assert!(!project_dir
            .as_path()
            .join("contracts")
            .join("alloc")
            .join("Makefile")
            .exists());
        assert!(!project_dir
            .as_path()
            .join("contracts")
            .join("alloc")
            .join("Cargo.lock")
            .exists());

        // check that the contract's Cargo.toml file uses the workspace for dependencies
        let contract_cargo_path = project_dir
            .as_path()
            .join("contracts")
            .join("alloc")
            .join("Cargo.toml");
        let cargo_toml_str = read_to_string(contract_cargo_path).unwrap();
        println!("{}", cargo_toml_str);

        assert!(cargo_toml_str.contains("soroban-sdk = { workspace = true }"));

        temp_dir.close().unwrap()
    }

    #[test]
    fn test_init_including_multiple_example_contracts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_examples = vec![ExampleContract::Account, ExampleContract::AtomicSwap];
        init(project_dir.as_path(), &with_examples).unwrap();

        assert!(project_dir
            .as_path()
            .join("contracts")
            .join("account")
            .exists());
        assert!(project_dir
            .as_path()
            .join("contracts")
            .join("atomic_swap")
            .exists());

        temp_dir.close().unwrap()
    }
}
