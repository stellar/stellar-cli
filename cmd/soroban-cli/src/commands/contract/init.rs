use core::fmt;
use std::fs::read_to_string;
use std::path::Path;
use std::{env, fs, io};

use clap::builder::{PossibleValue, PossibleValuesParser};
use clap::{Parser, ValueEnum};
use std::num::NonZeroU32;
use std::sync::atomic::AtomicBool;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use toml_edit::{Document, Formatted, InlineTable, TomlError, Value};

#[derive(Clone, Debug, PartialEq, ValueEnum, EnumIter)]

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
    Increment,
    LiquidityPool,
    Logging,
    SimpleAccount,
    SingleOffer,
    Timelock,
    Token,
    UpgradeableContract,
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
            ExampleContract::Increment => write!(f, "increment"),
            ExampleContract::LiquidityPool => write!(f, "liquidity_pool"),
            ExampleContract::Logging => write!(f, "logging"),
            ExampleContract::SimpleAccount => write!(f, "simple_account"),
            ExampleContract::SingleOffer => write!(f, "single_offer"),
            ExampleContract::Timelock => write!(f, "timelock"),
            ExampleContract::Token => write!(f, "token"),
            ExampleContract::UpgradeableContract => write!(f, "upgradeable_contract"),
        }
    }
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum FrontendTemplate {
    Astro,
    None,
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    pub project_path: String,

    /// An optional flag to specify Soroban example contracts to include. A hello-world contract will be included by default.
    #[arg(short, long, num_args = 1.., value_parser=possible_example_values())]
    pub with_example: Vec<String>,

    #[arg(short, long, value_enum, default_value = "none")]
    pub frontend_template: FrontendTemplate,
}

fn possible_example_values() -> PossibleValuesParser {
    //TODO: handle this unwrap more gracefully
    let examples = get_valid_examples().unwrap();
    let pvp = PossibleValuesParser::new(examples.iter().map(|s| PossibleValue::new(s)));

    pvp
}

fn get_valid_examples() -> Result<Vec<String>, Error> {
    let mut valid_examples = Vec::new();
    for example in ExampleContract::iter() {
        valid_examples.push(example.to_string());
    }

    Ok(valid_examples)
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
const FRONTEND_ASTRO_TEMPLATE_URL: &str = "https://github.com/AhaLabs/soroban-init-template";

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) -> Result<(), Error> {
        println!("ℹ️  Initializing project at {}", self.project_path);
        let project_path = Path::new(&self.project_path);

        init(project_path, &self.frontend_template, &self.with_example)?;

        Ok(())
    }
}

fn init(
    project_path: &Path,
    frontend_template: &FrontendTemplate,
    with_examples: &[String],
) -> Result<(), Error> {
    let cli_cmd_root = env!("CARGO_MANIFEST_DIR");
    let template_dir_path = Path::new(cli_cmd_root)
        .join("src")
        .join("utils")
        .join("contract-init-template");

    // create a project dir, and copy the contents of the base template (contract-init-template) into it
    std::fs::create_dir_all(project_path)?;
    copy_contents(template_dir_path.as_path(), project_path)?;

    if frontend_template != &FrontendTemplate::None {
        // create a temp dir for the template repo
        let fe_template_dir = tempfile::tempdir()?;

        // clone the template repo into the temp dir
        clone_repo(FRONTEND_ASTRO_TEMPLATE_URL, fe_template_dir.path())?;

        // copy the frontend template files into the project
        copy_frontend_files(fe_template_dir.path(), project_path, frontend_template);
    }

    // if there are --with-example flags, include the example contracts
    if include_example_contracts(with_examples) {
        // create an examples temp dir
        let examples_dir = tempfile::tempdir()?;

        // clone the soroban-examples repo into the temp dir
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
                //if file is .gitignore, merge the files
                if path.to_string_lossy().contains(".gitignore") {
                    let new_contents = read_to_string(&new_path)?;
                    let old_contents = read_to_string(&path)?;
                    let merged_contents = format!("{new_contents}\n{old_contents}");
                    std::fs::write(&new_path, merged_contents)?;
                    continue;
                }

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

fn include_example_contracts(contracts: &[String]) -> bool {
    !contracts.is_empty()
}

fn clone_repo(from_url: &str, to_path: &Path) -> Result<(), Error> {
    let mut prepare = gix::clone::PrepareFetch::new(
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

    let (mut checkout, _outcome) = prepare
        .fetch_then_checkout(gix::progress::Discard, &AtomicBool::new(false))
        .map_err(Box::new)?;

    let (_repo, _outcome) =
        checkout.main_worktree(gix::progress::Discard, &AtomicBool::new(false))?;

    Ok(())
}

fn copy_example_contracts(from: &Path, to: &Path, contracts: &[String]) -> Result<(), Error> {
    let project_contracts_path = to.join("contracts");
    for contract in contracts {
        println!("ℹ️  Initializing example contract: {contract}");
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

fn copy_frontend_files(from: &Path, to: &Path, template: &FrontendTemplate) {
    println!("ℹ️  Initializing with {template:?} frontend template");
    match template {
        FrontendTemplate::Astro => {
            let from_template_path = from.join("astro");
            let _ = copy_contents(&from_template_path, to);
        }
        FrontendTemplate::None => {}
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::read_to_string, path::PathBuf};

    use super::*;

    #[test]
    fn test_init() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_examples = vec![];
        init(
            project_dir.as_path(),
            &FrontendTemplate::None,
            &with_examples,
        )
        .unwrap();

        assert_base_template_files_exist(&project_dir);
        assert_default_hello_world_contract_files_exist(&project_dir);
        assert_base_excluded_paths_do_not_exist(&project_dir);

        // check that the contract's Cargo.toml file uses the workspace for dependencies
        assert_contract_cargo_file_uses_workspace(&project_dir, "hello_world");

        assert_base_excluded_paths_do_not_exist(&project_dir);

        temp_dir.close().unwrap();
    }

    #[test]
    fn test_init_including_example_contract() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_examples = vec![ExampleContract::Alloc];
        init(
            project_dir.as_path(),
            &FrontendTemplate::None,
            &with_examples,
        )
        .unwrap();

        assert_base_template_files_exist(&project_dir);
        assert_default_hello_world_contract_files_exist(&project_dir);
        assert_base_excluded_paths_do_not_exist(&project_dir);

        // check that alloc contract files exist
        assert_contract_files_exist(&project_dir, "alloc");

        // check that expected files are excluded from the alloc contract dir
        assert_example_contract_excluded_files_do_not_exist(&project_dir, "alloc");

        // check that the alloc contract's Cargo.toml file uses the workspace for dependencies
        assert_contract_cargo_file_uses_workspace(&project_dir, "alloc");

        temp_dir.close().unwrap();
    }

    #[test]
    fn test_init_including_multiple_example_contracts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_examples = vec![ExampleContract::Account, ExampleContract::AtomicSwap];
        init(
            project_dir.as_path(),
            &FrontendTemplate::None,
            &with_examples,
        )
        .unwrap();

        assert_base_template_files_exist(&project_dir);
        assert_default_hello_world_contract_files_exist(&project_dir);
        assert_base_excluded_paths_do_not_exist(&project_dir);

        // check that account contract files exist and that expected files are excluded
        assert_contract_files_exist(&project_dir, "account");
        assert_example_contract_excluded_files_do_not_exist(&project_dir, "account");
        assert_contract_cargo_file_uses_workspace(&project_dir, "account");

        // check that atomic_swap contract files exist and that expected files are excluded
        assert_contract_files_exist(&project_dir, "atomic_swap");
        assert_example_contract_excluded_files_do_not_exist(&project_dir, "atomic_swap");
        assert_contract_cargo_file_uses_workspace(&project_dir, "atomic_swap");

        temp_dir.close().unwrap();
    }

    #[test]
    fn test_init_with_frontend_template() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_examples = vec![];
        init(
            project_dir.as_path(),
            &FrontendTemplate::Astro,
            &with_examples,
        )
        .unwrap();

        assert_base_template_files_exist(&project_dir);
        assert_default_hello_world_contract_files_exist(&project_dir);
        assert_base_excluded_paths_do_not_exist(&project_dir);

        // check that the contract's Cargo.toml file uses the workspace for dependencies
        assert_contract_cargo_file_uses_workspace(&project_dir, "hello_world");
        assert_base_excluded_paths_do_not_exist(&project_dir);

        assert_astro_files_exist(&project_dir);

        assert_gitignore_includes_astro_paths(&project_dir);

        temp_dir.close().unwrap();
    }

    // test helpers
    fn assert_base_template_files_exist(project_dir: &PathBuf) {
        let expected_paths = ["contracts", "Cargo.toml", "README.md"];
        for path in expected_paths.iter() {
            assert!(project_dir.join(path).exists());
        }
    }

    fn assert_default_hello_world_contract_files_exist(project_dir: &PathBuf) {
        assert_contract_files_exist(project_dir, "hello_world");
    }

    fn assert_contract_files_exist(project_dir: &PathBuf, contract_name: &str) {
        let contract_dir = project_dir.as_path().join("contracts").join(contract_name);

        assert!(contract_dir.exists());
        assert!(contract_dir.as_path().join("Cargo.toml").exists());
        assert!(contract_dir.as_path().join("src").join("lib.rs").exists());
        assert!(contract_dir.as_path().join("src").join("test.rs").exists());
    }

    fn assert_contract_cargo_file_uses_workspace(project_dir: &PathBuf, contract_name: &str) {
        let contract_dir = project_dir.as_path().join("contracts").join(contract_name);
        let cargo_toml_path = contract_dir.as_path().join("Cargo.toml");
        let cargo_toml_str = read_to_string(cargo_toml_path).unwrap();
        assert!(cargo_toml_str.contains("soroban-sdk = { workspace = true }"));
    }

    fn assert_example_contract_excluded_files_do_not_exist(
        project_dir: &PathBuf,
        contract_name: &str,
    ) {
        let contract_dir = project_dir.as_path().join("contracts").join(contract_name);
        assert!(!contract_dir.as_path().join("Makefile").exists());
        assert!(!contract_dir.as_path().join("Cargo.lock").exists());
    }

    fn assert_base_excluded_paths_do_not_exist(project_dir: &PathBuf) {
        let excluded_paths = [
            ".git",
            ".github",
            "Makefile",
            "Cargo.lock",
            ".vscode",
            "target",
        ];
        for path in excluded_paths.iter() {
            assert!(!project_dir.join(path).exists());
        }
    }

    fn assert_gitignore_includes_astro_paths(project_dir: &PathBuf) {
        let gitignore_path = project_dir.as_path().join(".gitignore");
        let gitignore_str = read_to_string(gitignore_path).unwrap();
        assert!(gitignore_str.contains(".astro/"));
        assert!(gitignore_str.contains("node_modules"));
        assert!(gitignore_str.contains("npm-debug.log*"));
    }

    fn assert_astro_files_exist(project_dir: &PathBuf) {
        assert!(project_dir.as_path().join("public").exists());
        assert!(project_dir.as_path().join("src").exists());
        assert!(project_dir
            .as_path()
            .join("src")
            .join("components")
            .exists());
        assert!(project_dir.as_path().join("src").join("layouts").exists());
        assert!(project_dir.as_path().join("src").join("pages").exists());
        assert!(project_dir.as_path().join("astro.config.mjs").exists());
        assert!(project_dir.as_path().join("tsconfig.json").exists());
    }
}
