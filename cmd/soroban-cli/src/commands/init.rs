use std::path::Path;
use std::{fs, io};

use clap::Parser;
use std::num::NonZeroU32;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Debug, PartialEq, clap::ValueEnum)]
pub enum ExampleContract {
    HelloWorld,
    Account,
    Alloc,
    None,
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    pub project_path: String,

    /// optional flag to specify the initial soroban example contracts to include
    #[arg(short, long, num_args = 1..=6, default_value = "none")]
    pub with_contract: Vec<ExampleContract>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to create directory: {0}")]
    CreateDirError(#[from] io::Error),

    #[error("Failed to clone the template repository: {0}")]
    CloneError(#[from] gix::clone::Error),

    #[error("Failed to fetch the template repository: {0}")]
    FetchError(#[from] gix::clone::fetch::Error),

    #[error("Failed to checkout the template repository: {0}")]
    CheckoutError(#[from] gix::clone::checkout::main_worktree::Error),
}

const TEMPLATE_URL: &str = "https://github.com/AhaLabs/soroban-tutorial-project.git";
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
        println!("Including example contracts: {:?}", with_contracts);

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
        let contract_dir = match contract {
            ExampleContract::HelloWorld => Path::new("hello-world"),
            ExampleContract::Alloc => Path::new("alloc"),
            ExampleContract::Account => Path::new("account"),
            ExampleContract::None => continue,
        };

        let from_contract_path = from.join(contract_dir);
        let to_contract_path = project_contracts_path.join(contract_dir);
        std::fs::create_dir_all(&to_contract_path)?;

        copy_contents(&from_contract_path, &to_contract_path)?
    }
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
    )?
    .with_shallow(gix::remote::fetch::Shallow::DepthAtRemote(
        NonZeroU32::new(1).unwrap(),
    ));

    let (mut prepare, _outcome) =
        fetch.fetch_then_checkout(gix::progress::Discard, &AtomicBool::new(false))?;

    let (_repo, _outcome) =
        prepare.main_worktree(gix::progress::Discard, &AtomicBool::new(false))?;

    Ok(())
}

fn copy_contents(from: &Path, to: &Path) -> Result<(), Error> {
    let default_entries_to_exclude = vec![".git", ".github", "Makefile", "Cargo.lock"];
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let entry_name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        let file_name = path.file_name().unwrap();
        let new_path = to.join(file_name);

        if default_entries_to_exclude.contains(&entry_name.as_str()) {
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
    use super::*;

    #[test]
    fn test_init() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_contracts = vec![ExampleContract::None];
        init(project_dir.as_path(), TEMPLATE_URL, &with_contracts).unwrap();

        assert!(project_dir.as_path().join("README.md").exists());
        assert!(project_dir.as_path().join("contracts").exists());

        // check that it does not include certain template files
        assert!(!project_dir.as_path().join(".git").exists());
        assert!(!project_dir.as_path().join(".github").exists());
        assert!(!project_dir.as_path().join("Cargo.lock").exists());

        temp_dir.close().unwrap()
    }

    #[test]
    fn test_include_contract() {
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

        // check that it does not include certain template files
        assert!(!project_dir.as_path().join(".git").exists());
        assert!(!project_dir.as_path().join(".github").exists());
        assert!(!project_dir.as_path().join("Cargo.lock").exists());

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

        temp_dir.close().unwrap()
    }
}
