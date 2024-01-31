use std::fs::read_to_string;
use std::path::Path;
use std::{env, fs, io};

use clap::builder::{PossibleValue, PossibleValuesParser, ValueParser};
use clap::{Parser, ValueEnum};
use serde::Deserialize;
use std::num::NonZeroU32;
use std::sync::atomic::AtomicBool;
use toml_edit::{Document, Formatted, InlineTable, TomlError, Value};

const SOROBAN_EXAMPLES_URL: &str = "https://github.com/stellar/soroban-examples.git";
const FRONTEND_ASTRO_TEMPLATE_URL: &str = "https://github.com/AhaLabs/soroban-astro-template";
const GITHUB_URL: &str = "https://github.com";
const GITHUB_API_URL: &str =
    "https://api.github.com/repos/stellar/soroban-examples/git/trees/main?recursive=1";

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

    #[arg(short, long, value_enum, default_value = "none")]
    pub frontend_template: FrontendTemplate,
}

fn possible_example_values() -> ValueParser {
    // If fetching the example contracts from the soroban-examples repo succeeds, return a parser with the example contracts.
    if let Ok(examples) = get_valid_examples() {
        let parser = PossibleValuesParser::new(examples.iter().map(PossibleValue::new));
        return parser.into();
    }

    // If fetching with example contracts fails, return a string parser that will allow for any value. It will be ignored in `init`.
    ValueParser::string()
}

fn with_example_help() -> String {
    if check_internet_connection() {
        "An optional flag to specify Soroban example contracts to include. A hello-world contract will be included by default.".to_owned()
    } else {
        "⚠️  Failed to fetch additional example contracts from soroban-examples repo. You can still continue with initializing - the default hello_world contract will still be included".to_owned()
    }
}

#[derive(Deserialize, Debug)]
struct RepoPath {
    path: String,
    #[serde(rename = "type")]
    type_field: String,
}

#[derive(Deserialize, Debug)]
struct ReqBody {
    tree: Vec<RepoPath>,
}

fn get_valid_examples() -> Result<Vec<String>, Error> {
    let body: ReqBody = ureq::get(GITHUB_API_URL)
        .call()
        .map_err(Box::new)?
        .into_json()?;
    let mut valid_examples = Vec::new();
    for item in body.tree {
        if item.type_field == "blob"
            || item.path.starts_with('.')
            || item.path.contains('/')
            || item.path == "hello_world"
        {
            continue;
        }

        valid_examples.push(item.path);
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

    #[error("Failed to fetch example contracts")]
    ExampleContractFetchError(#[from] Box<ureq::Error>),

    #[error("Failed to parse package.json file: {0}")]
    JsonParseError(#[from] serde_json::Error),
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

    if !check_internet_connection() {
        println!("⚠️  It doesn't look like you're connected to the internet. We're still able to initialize a new project, but additional examples and the frontend template will not be included.");
        return Ok(());
    }

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
                //if file is .gitignore, overwrite the file with a new .gitignore file
                if path.to_string_lossy().contains(".gitignore") {
                    std::fs::copy(&path, &new_path)?;
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
            let _ = copy_contents(from, to);
            let _ = edit_package_json_files(to);
        }
        FrontendTemplate::None => {}
    }
}

fn edit_package_json_files(project_path: &Path) -> Result<(), Error> {
    let package_name = project_path.file_name().unwrap();
    edit_package_json(project_path, package_name)?;
    edit_package_lock_json(project_path, package_name)
}

fn edit_package_lock_json(
    project_path: &Path,
    package_name: &std::ffi::OsStr,
) -> Result<(), Error> {
    let package_lock_json_path = project_path.join("package-lock.json");
    let package_lock_json_str = read_to_string(&package_lock_json_path)?;

    let mut doc: serde_json::Value = serde_json::from_str(&package_lock_json_str)?;

    doc["name"] = serde_json::json!(package_name.to_string_lossy());

    std::fs::write(&package_lock_json_path, doc.to_string())?;

    Ok(())
}

fn edit_package_json(project_path: &Path, package_name: &std::ffi::OsStr) -> Result<(), Error> {
    let package_json_path = project_path.join("package.json");
    let package_json_str = read_to_string(&package_json_path)?;

    let mut doc: serde_json::Value = serde_json::from_str(&package_json_str)?;

    doc["name"] = serde_json::json!(package_name.to_string_lossy());

    std::fs::write(&package_json_path, doc.to_string())?;

    Ok(())
}

fn check_internet_connection() -> bool {
    if let Ok(_req) = ureq::get(GITHUB_URL).call() {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;

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
        let with_examples = ["alloc".to_owned()];
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
        let with_examples = ["account".to_owned(), "atomic_swap".to_owned()];
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
    fn test_init_with_invalid_example_contract() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let with_examples = ["invalid_example".to_owned(), "atomic_swap".to_owned()];
        assert!(init(
            project_dir.as_path(),
            &FrontendTemplate::None,
            &with_examples,
        )
        .is_err());

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
        assert!(!contract_dir.as_path().join("Cargo.lock").exists());
    }

    fn assert_base_excluded_paths_do_not_exist(project_dir: &Path) {
        let excluded_paths = [
            ".git",
            ".github",
            "Makefile",
            "Cargo.lock",
            ".vscode",
            "target",
        ];
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
}
