use std::fs::read_to_string;
use std::path::Path;
use std::{fs, io};

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

impl ToString for ExampleContract {
    fn to_string(&self) -> String {
        match self {
            ExampleContract::Account => String::from("account"),
            ExampleContract::Alloc => String::from("alloc"),
            ExampleContract::AtomicMultiswap => String::from("atomic_multiswap"),
            ExampleContract::AtomicSwap => String::from("atomic_swap"),
            ExampleContract::Auth => String::from("auth"),
            ExampleContract::CrossContract => String::from("cross_contract"),
            ExampleContract::CustomTypes => String::from("custom_types"),
            ExampleContract::DeepContractAuth => String::from("deep_contract_auth"),
            ExampleContract::Deployer => String::from("deployer"),
            ExampleContract::Errors => String::from("errors"),
            ExampleContract::Events => String::from("events"),
            ExampleContract::Fuzzing => String::from("fuzzing"),
            ExampleContract::HelloWorld => String::from("hello_world"),
            ExampleContract::Increment => String::from("increment"),
            ExampleContract::LiquidityPool => String::from("liquidity_pool"),
            ExampleContract::Logging => String::from("logging"),
            ExampleContract::SimpleAccount => String::from("simple_account"),
            ExampleContract::SingleOffer => String::from("single_offer"),
            ExampleContract::Timelock => String::from("timelock"),
            ExampleContract::Token => String::from("token"),
            ExampleContract::UpgradeableContract => String::from("upgradeable_contract"),
            ExampleContract::None => String::from("none"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    pub project_path: String,

    /// optional flag to specify soroban example contracts to include
    #[arg(short, long, num_args = 1..=20, default_value = "none")]
    pub with_contract: Vec<ExampleContract>,
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

const TEMPLATE_URL: &str = "https://github.com/AhaLabs/soroban-init-template.git";
const SOROBAN_EXAMPLES_URL: &str = "https://github.com/stellar/soroban-examples.git";

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) -> Result<(), Error> {
        println!("Creating a new soroban project at {}", self.project_path);
        let project_path = Path::new(&self.project_path);

        init(project_path, TEMPLATE_URL, &self.with_contract)?;

        Ok(())
    }
}

fn init(
    project_path: &Path,
    template_url: &str,
    with_contracts: &[ExampleContract],
) -> Result<(), Error> {
    // create a template temp dir to clone the template repo into
    let template_dir = tempfile::tempdir()?;

    // clone the template repo into the temp dir
    clone_repo(template_url, template_dir.path())?;

    // create the project directory and copy the template contents into it
    std::fs::create_dir_all(project_path)?;
    copy_contents(template_dir.path(), project_path)?;

    // if there are with-contract flags, include the example contracts
    if include_example_contracts(with_contracts) {
        println!("Including example contracts: {with_contracts:?}");

        // create an examples temp dir in the temp dir
        let examples_dir = tempfile::tempdir()?;

        // clone the soroban-examples repo into temp dir
        clone_repo(SOROBAN_EXAMPLES_URL, examples_dir.path())?;

        // copy the example contracts into the project
        copy_example_contracts(examples_dir.path(), project_path, with_contracts)?;
    }

    Ok(())
}

fn copy_example_contracts(
    from: &Path,
    to: &Path,
    contracts: &[ExampleContract],
) -> Result<(), Error> {
    let project_contracts_path = to.join("contracts");
    for contract in contracts {
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

fn copy_contents(from: &Path, to: &Path) -> Result<(), Error> {
    let contents_to_exclude_from_copy = [".git", ".github", "Makefile", "Cargo.lock", ".vscode"];
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
            std::fs::copy(&path, &new_path)?;
        }
    }

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
        let with_contracts = vec![ExampleContract::None];
        init(project_dir.as_path(), TEMPLATE_URL, &with_contracts).unwrap();

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
        let with_contracts = vec![ExampleContract::Alloc];
        init(project_dir.as_path(), TEMPLATE_URL, &with_contracts).unwrap();

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
        let with_contracts = vec![ExampleContract::Account, ExampleContract::AtomicSwap];
        init(project_dir.as_path(), TEMPLATE_URL, &with_contracts).unwrap();

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
