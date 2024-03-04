use std::{
    ffi::OsStr,
    fs::{copy, create_dir_all, metadata, read_dir, read_to_string, write},
    io,
    num::NonZeroU32,
    path::Path,
    str,
    sync::atomic::AtomicBool,
};

use clap::{
    builder::{PossibleValue, PossibleValuesParser, ValueParser},
    Parser, ValueEnum,
};
use gix::{clone, create, open, progress, remote};
use rust_embed::RustEmbed;
use serde_json::{from_str, json, Error as JsonError, Value as JsonValue};
use toml_edit::{Document, Formatted, InlineTable, Item, TomlError, Value as TomlValue};
use ureq::{get, Error as UreqError};

const SOROBAN_EXAMPLES_URL: &str = "https://github.com/stellar/soroban-examples.git";
const GITHUB_URL: &str = "https://github.com";

#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum FrontendTemplate {
    Astro,
    None,
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    pub project_path: String,

    #[arg(short, long, num_args = 1.., value_parser=possible_example_values(), long_help=with_example_help())]
    pub with_example: Vec<String>,

    #[arg(
        short,
        long,
        default_value = "",
        long_help = "An optional flag to pass in a url for a frontend template repository."
    )]
    pub frontend_template: String,
}

fn possible_example_values() -> ValueParser {
    let parser = PossibleValuesParser::new(
        [
            "account",
            "alloc",
            "atomic_multiswap",
            "atomic_swap",
            "auth",
            "cross_contract",
            "custom_types",
            "deep_contract_auth",
            "deployer",
            "errors",
            "eth_abi",
            "events",
            "fuzzing",
            "increment",
            "liquidity_pool",
            "logging",
            "mint-lock",
            "simple_account",
            "single_offer",
            "timelock",
            "token",
            "upgradeable_contract",
            "workspace",
        ]
        .iter()
        .map(PossibleValue::new),
    );
    parser.into()
}

fn with_example_help() -> String {
    if check_internet_connection() {
        "An optional flag to specify Soroban example contracts to include. A hello-world contract will be included by default.".to_owned()
    } else {
        "⚠️  Failed to fetch additional example contracts from soroban-examples repo. You can continue with initializing - the default hello_world contract will still be included".to_owned()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Io error: {0}")]
    IoError(#[from] io::Error),

    // the gix::clone::Error is too large to include in the error enum as is, so we wrap it in a Box
    #[error("Failed to clone repository: {0}")]
    CloneError(#[from] Box<clone::Error>),

    // the gix::clone::fetch::Error is too large to include in the error enum as is, so we wrap it in a Box
    #[error("Failed to fetch repository: {0}")]
    FetchError(#[from] Box<clone::fetch::Error>),

    #[error("Failed to checkout repository worktree: {0}")]
    CheckoutError(#[from] clone::checkout::main_worktree::Error),

    #[error("Failed to parse toml file: {0}")]
    TomlParseError(#[from] TomlError),

    #[error("Failed to complete get request")]
    UreqError(#[from] Box<UreqError>),

    #[error("Failed to parse package.json file: {0}")]
    JsonParseError(#[from] JsonError),

    #[error("Failed to convert bytes to string: {0}")]
    ConverBytesToStringErr(#[from] str::Utf8Error),
}

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) -> Result<(), Error> {
        println!("ℹ️  Initializing project at {}", self.project_path);
        let project_path = Path::new(&self.project_path);

        init(project_path, &self.frontend_template, &self.with_example)?;

        Ok(())
    }
}

#[derive(RustEmbed)]
#[folder = "src/utils/contract-init-template"]
struct TemplateFiles;

fn init(
    project_path: &Path,
    frontend_template: &str,
    with_examples: &[String],
) -> Result<(), Error> {
    // create a project dir, and copy the contents of the base template (contract-init-template) into it
    create_dir_all(project_path).map_err(|e| {
        eprintln!("Error creating new project directory: {project_path:?}");
        e
    })?;
    copy_template_files(project_path)?;

    if !check_internet_connection() {
        println!("⚠️  It doesn't look like you're connected to the internet. We're still able to initialize a new project, but additional examples and the frontend template will not be included.");
        return Ok(());
    }

    if !frontend_template.is_empty() {
        // create a temp dir for the template repo
        let fe_template_dir = tempfile::tempdir().map_err(|e| {
            eprintln!("Error creating temp dir for frontend template");
            e
        })?;

        // clone the template repo into the temp dir
        clone_repo(frontend_template, fe_template_dir.path())?;

        // copy the frontend template files into the project
        copy_frontend_files(fe_template_dir.path(), project_path)?;
    }

    // if there are --with-example flags, include the example contracts
    if include_example_contracts(with_examples) {
        // create an examples temp dir
        let examples_dir = tempfile::tempdir().map_err(|e| {
            eprintln!("Error creating temp dir for soroban-examples");
            e
        })?;

        // clone the soroban-examples repo into the temp dir
        clone_repo(SOROBAN_EXAMPLES_URL, examples_dir.path())?;

        // copy the example contracts into the project
        copy_example_contracts(examples_dir.path(), project_path, with_examples)?;
    }

    Ok(())
}

fn copy_template_files(project_path: &Path) -> Result<(), Error> {
    for item in TemplateFiles::iter() {
        let mut to = project_path.join(item.as_ref());

        if file_exists(&to.to_string_lossy()) {
            println!(
                "ℹ️  Skipped creating {} as it already exists",
                &to.to_string_lossy()
            );
            continue;
        }
        create_dir_all(to.parent().unwrap()).map_err(|e| {
            eprintln!("Error creating directory path for: {to:?}");
            e
        })?;

        let Some(file) = TemplateFiles::get(item.as_ref()) else {
            println!("⚠️  Failed to read file: {}", item.as_ref());
            continue;
        };

        let file_contents = std::str::from_utf8(file.data.as_ref()).map_err(|e| {
            eprintln!(
                "Error converting file contents in {:?} to string",
                item.as_ref()
            );
            e
        })?;

        // We need to include the Cargo.toml file as Cargo.text in the template so that it will be included the package. This is making sure that the Cargo file is written as Cargo.toml in the new project. This is a workaround for this issue: https://github.com/rust-lang/cargo/issues/8597.
        if item == "Cargo.toml.removeextension" {
            to = project_path.join("Cargo.toml");
        }

        println!("➕  Writing {}", &to.to_string_lossy());
        write(&to, file_contents).map_err(|e| {
            eprintln!("Error writing file: {to:?}");
            e
        })?;
    }
    Ok(())
}

fn copy_contents(from: &Path, to: &Path) -> Result<(), Error> {
    let contents_to_exclude_from_copy = [".git", ".github", "Makefile", ".vscode", "target"];
    for entry in read_dir(from).map_err(|e| {
        eprintln!("Error reading directory: {from:?}");
        e
    })? {
        let entry = entry.map_err(|e| {
            eprintln!("Error reading entry in directory: {from:?}");
            e
        })?;
        let path = entry.path();
        let entry_name = entry.file_name().to_string_lossy().to_string();
        let new_path = to.join(&entry_name);

        if contents_to_exclude_from_copy.contains(&entry_name.as_str()) {
            continue;
        }

        if path.is_dir() {
            create_dir_all(&new_path).map_err(|e| {
                eprintln!("Error creating directory: {new_path:?}");
                e
            })?;
            copy_contents(&path, &new_path)?;
        } else {
            if file_exists(&new_path.to_string_lossy()) {
                //if file is .gitignore, overwrite the file with a new .gitignore file
                if path.to_string_lossy().contains(".gitignore") {
                    copy(&path, &new_path).map_err(|e| {
                        eprintln!(
                            "Error copying from {:?} to {:?}",
                            path.to_string_lossy(),
                            new_path
                        );

                        e
                    })?;
                    continue;
                }

                println!(
                    "ℹ️  Skipped creating {} as it already exists",
                    &new_path.to_string_lossy()
                );
                continue;
            }

            println!("➕  Writing {}", &new_path.to_string_lossy());
            copy(&path, &new_path).map_err(|e| {
                eprintln!(
                    "Error copying from {:?} to {:?}",
                    path.to_string_lossy(),
                    new_path
                );
                e
            })?;
        }
    }

    Ok(())
}

fn file_exists(file_path: &str) -> bool {
    if let Ok(metadata) = metadata(file_path) {
        metadata.is_file()
    } else {
        false
    }
}

fn include_example_contracts(contracts: &[String]) -> bool {
    !contracts.is_empty()
}

fn clone_repo(from_url: &str, to_path: &Path) -> Result<(), Error> {
    let mut prepare = clone::PrepareFetch::new(
        from_url,
        to_path,
        create::Kind::WithWorktree,
        create::Options {
            destination_must_be_empty: false,
            fs_capabilities: None,
        },
        open::Options::isolated(),
    )
    .map_err(|e| {
        eprintln!("Error preparing fetch for {from_url:?}");
        Box::new(e)
    })?
    .with_shallow(remote::fetch::Shallow::DepthAtRemote(
        NonZeroU32::new(1).unwrap(),
    ));

    let (mut checkout, _outcome) = prepare
        .fetch_then_checkout(progress::Discard, &AtomicBool::new(false))
        .map_err(|e| {
            eprintln!("Error calling fetch_then_checkout with {from_url:?}");
            Box::new(e)
        })?;

    let (_repo, _outcome) = checkout
        .main_worktree(progress::Discard, &AtomicBool::new(false))
        .map_err(|e| {
            eprintln!("Error calling main_worktree for {from_url:?}");
            e
        })?;

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
        create_dir_all(&to_contract_path).map_err(|e| {
            eprintln!("Error creating directory: {contract_path:?}");
            e
        })?;

        copy_contents(&from_contract_path, &to_contract_path)?;
        edit_contract_cargo_file(&to_contract_path)?;
    }

    Ok(())
}

fn edit_contract_cargo_file(contract_path: &Path) -> Result<(), Error> {
    let cargo_path = contract_path.join("Cargo.toml");
    let cargo_toml_str = read_to_string(&cargo_path).map_err(|e| {
        eprint!("Error reading Cargo.toml file in: {contract_path:?}");
        e
    })?;
    let mut doc = cargo_toml_str.parse::<Document>().map_err(|e| {
        eprintln!("Error parsing Cargo.toml file in: {contract_path:?}");
        e
    })?;

    let mut workspace_table = InlineTable::new();
    workspace_table.insert("workspace", TomlValue::Boolean(Formatted::new(true)));

    doc["dependencies"]["soroban-sdk"] =
        Item::Value(TomlValue::InlineTable(workspace_table.clone()));
    doc["dev_dependencies"]["soroban-sdk"] = Item::Value(TomlValue::InlineTable(workspace_table));

    doc.remove("profile");

    write(&cargo_path, doc.to_string()).map_err(|e| {
        eprintln!("Error writing to Cargo.toml file in: {contract_path:?}");
        e
    })?;

    Ok(())
}

fn copy_frontend_files(from: &Path, to: &Path) -> Result<(), Error> {
    println!("ℹ️  Initializing with frontend template");
    copy_contents(from, to)?;
    edit_package_json_files(to)
}

fn edit_package_json_files(project_path: &Path) -> Result<(), Error> {
    let package_name = project_path.file_name().unwrap();
    edit_package_name(project_path, package_name, "package.json").map_err(|e| {
        eprintln!("Error editing package.json file in: {project_path:?}");
        e
    })?;
    edit_package_name(project_path, package_name, "package-lock.json")
}

fn edit_package_name(
    project_path: &Path,
    package_name: &OsStr,
    file_name: &str,
) -> Result<(), Error> {
    let file_path = project_path.join(file_name);
    let file_contents = read_to_string(&file_path)?;

    let mut doc: JsonValue = from_str(&file_contents).map_err(|e| {
        eprintln!("Error parsing package.json file in: {project_path:?}");
        e
    })?;

    doc["name"] = json!(package_name.to_string_lossy());

    write(&file_path, doc.to_string())?;

    Ok(())
}

fn check_internet_connection() -> bool {
    if let Ok(_req) = get(GITHUB_URL).call() {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;

    use super::*;

    const TEST_PROJECT_NAME: &str = "test-project";

    #[test]
    fn test_init() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join(TEST_PROJECT_NAME);
        let with_examples = vec![];
        init(project_dir.as_path(), "", &with_examples).unwrap();

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
        let project_dir = temp_dir.path().join(TEST_PROJECT_NAME);
        let with_examples = ["alloc".to_owned()];
        init(project_dir.as_path(), "", &with_examples).unwrap();

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
        let with_examples = ["account".to_owned(), "atomic_swap".to_owned()];
        init(project_dir.as_path(), "", &with_examples).unwrap();

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
    fn test_init_with_invalid_example_contract() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_examples = ["invalid_example".to_owned(), "atomic_swap".to_owned()];
        assert!(init(project_dir.as_path(), "", &with_examples,).is_err());

        temp_dir.close().unwrap();
    }

    #[test]
    fn test_init_with_frontend_template() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join(TEST_PROJECT_NAME);
        let with_examples = vec![];
        init(
            project_dir.as_path(),
            "https://github.com/AhaLabs/soroban-astro-template",
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
        assert_package_json_files_have_correct_name(&project_dir);

        temp_dir.close().unwrap();
    }

    // test helpers
    fn assert_base_template_files_exist(project_dir: &Path) {
        let expected_paths = ["contracts", "Cargo.toml", "README.md"];
        for path in &expected_paths {
            assert!(project_dir.join(path).exists());
        }
    }

    fn assert_default_hello_world_contract_files_exist(project_dir: &Path) {
        assert_contract_files_exist(project_dir, "hello_world");
    }

    fn assert_contract_files_exist(project_dir: &Path, contract_name: &str) {
        let contract_dir = project_dir.join("contracts").join(contract_name);

        assert!(contract_dir.exists());
        assert!(contract_dir.as_path().join("Cargo.toml").exists());
        assert!(contract_dir.as_path().join("src").join("lib.rs").exists());
        assert!(contract_dir.as_path().join("src").join("test.rs").exists());
    }

    fn assert_contract_cargo_file_uses_workspace(project_dir: &Path, contract_name: &str) {
        let contract_dir = project_dir.join("contracts").join(contract_name);
        let cargo_toml_path = contract_dir.as_path().join("Cargo.toml");
        let cargo_toml_str = read_to_string(cargo_toml_path).unwrap();
        assert!(cargo_toml_str.contains("soroban-sdk = { workspace = true }"));
    }

    fn assert_example_contract_excluded_files_do_not_exist(
        project_dir: &Path,
        contract_name: &str,
    ) {
        let contract_dir = project_dir.join("contracts").join(contract_name);
        assert!(!contract_dir.as_path().join("Makefile").exists());
    }

    fn assert_base_excluded_paths_do_not_exist(project_dir: &Path) {
        let excluded_paths = [".git", ".github", "Makefile", ".vscode", "target"];
        for path in &excluded_paths {
            assert!(!project_dir.join(path).exists());
        }
    }

    fn assert_gitignore_includes_astro_paths(project_dir: &Path) {
        let gitignore_path = project_dir.join(".gitignore");
        let gitignore_str = read_to_string(gitignore_path).unwrap();
        assert!(gitignore_str.contains(".astro/"));
        assert!(gitignore_str.contains("node_modules"));
        assert!(gitignore_str.contains("npm-debug.log*"));
    }

    fn assert_astro_files_exist(project_dir: &Path) {
        assert!(project_dir.join("public").exists());
        assert!(project_dir.join("src").exists());
        assert!(project_dir.join("src").join("components").exists());
        assert!(project_dir.join("src").join("layouts").exists());
        assert!(project_dir.join("src").join("pages").exists());
        assert!(project_dir.join("astro.config.mjs").exists());
        assert!(project_dir.join("tsconfig.json").exists());
    }

    fn assert_package_json_files_have_correct_name(project_dir: &Path) {
        let package_json_path = project_dir.join("package.json");
        let package_json_str = read_to_string(package_json_path).unwrap();
        assert!(package_json_str.contains(&format!("\"name\":\"{TEST_PROJECT_NAME}\"")));

        let package_lock_json_path = project_dir.join("package-lock.json");
        let package_lock_json_str = read_to_string(package_lock_json_path).unwrap();
        assert!(package_lock_json_str.contains(&format!("\"name\":\"{TEST_PROJECT_NAME}\"")));
    }
}
