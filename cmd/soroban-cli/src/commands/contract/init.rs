use std::{
    fs::{create_dir_all, write},
    io,
    path::{Path, PathBuf},
    str,
};

use clap::Parser;
use rust_embed::RustEmbed;

use crate::commands::contract::init::Error::{
    AlreadyExists, PathExistsNotCargoProject, PathExistsNotDir,
};
use crate::{commands::global, print};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    pub project_path: String,

    #[arg(
        long,
        default_value = "hello-world",
        long_help = "An optional flag to specify a new contract's name."
    )]
    pub name: String,

    // TODO: remove in 23.0
    #[arg(
        short,
        long,
        action = clap::ArgAction::HelpLong,
        long_help = "This argument has been deprecated and will be removed in the future versions of CLI. You can still clone examples from the repo https://github.com/stellar/soroban-examples",
    )]
    pub with_example: Option<String>,

    // TODO: remove in 23.0
    #[arg(
        long,
        action = clap::ArgAction::HelpLong,
        long_help = "This argument has been deprecated and will be removed in the future versions of CLI. You can search for frontend templates using github tags, such as soroban-template or soroban-frontend-template",
    )]
    pub frontend_template: Option<String>,

    // TODO: remove in 23.0
    #[arg(
        long,
        action = clap::ArgAction::HelpLong,
        long_help = "This argument has been deprecated and will be removed in the future versions of CLI. init command no longer overwrites existing files."
    )]
    pub overwrite: Option<bool>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(String, io::Error),

    #[error(transparent)]
    Std(#[from] std::io::Error),

    #[error("failed to convert bytes to string: {0}")]
    ConvertBytesToString(#[from] str::Utf8Error),

    #[error("contract package already exists: {0}")]
    AlreadyExists(String),

    #[error("provided project path exists and is not a directory")]
    PathExistsNotDir,

    #[error("provided project path exists and is not a cargo workspace root directory. Hint: run init on an empty or non-existing directory"
    )]
    PathExistsNotCargoProject,
}

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let runner = Runner {
            args: self.clone(),
            print: print::Print::new(global_args.quiet),
        };

        runner.run()
    }
}

#[derive(RustEmbed)]
#[folder = "src/utils/contract-workspace-template"]
struct WorkspaceTemplate;

#[derive(RustEmbed)]
#[folder = "src/utils/contract-template"]
struct ContractTemplate;

struct Runner {
    args: Cmd,
    print: print::Print,
}

impl Runner {
    fn run(&self) -> Result<(), Error> {
        let project_path = PathBuf::from(&self.args.project_path);

        if project_path.exists() {
            if project_path.is_dir() {
                if project_path.read_dir()?.next().is_none() {
                    self.init_workspace()?;
                } else if !project_path.join("Cargo.toml").exists() {
                    return Err(PathExistsNotCargoProject);
                }
            } else {
                return Err(PathExistsNotDir);
            }
        } else {
            self.init_workspace()?;
        }

        self.copy_template_files()?;

        Ok(())
    }

    fn init_workspace(&self) -> Result<(), Error> {
        let project_path = Path::new(&self.args.project_path);

        self.print
            .infoln(format!("Initializing workspace at {project_path:?}"));

        for item in WorkspaceTemplate::iter() {
            let to = project_path.join(item.as_ref());
            Self::create_dir_all(to.parent().unwrap())?;

            let Some(file) = WorkspaceTemplate::get(item.as_ref()) else {
                self.print
                    .warnln(format!("Failed to read file: {}", item.as_ref()));
                continue;
            };

            let file_contents =
                str::from_utf8(file.data.as_ref()).map_err(Error::ConvertBytesToString)?;

            Self::write(&to, file_contents)?;
        }

        Self::create_dir_all(project_path.join("contracts").as_path())?;

        Ok(())
    }

    fn copy_template_files(&self) -> Result<(), Error> {
        let binding = Path::new(&self.args.project_path)
            .join("contracts")
            .join(&self.args.name);
        let project_path = binding.as_path();

        self.print.infoln(format!(
            "Adding package to the workspace at {project_path:?}"
        ));

        if project_path.exists() {
            return Err(AlreadyExists(self.args.name.clone()));
        }

        Self::create_dir_all(project_path)?;

        for item in ContractTemplate::iter() {
            let mut to = project_path.join(item.as_ref());
            Self::create_dir_all(to.parent().unwrap())?;

            let Some(file) = ContractTemplate::get(item.as_ref()) else {
                self.print
                    .warnln(format!("Failed to read file: {}", item.as_ref()));
                continue;
            };

            // We need to include the Cargo.toml file as Cargo.toml.removeextension in the template so that it will be included the package. This is making sure that the Cargo file is written as Cargo.toml in the new project. This is a workaround for this issue: https://github.com/rust-lang/cargo/issues/8597.
            let item_path = Path::new(item.as_ref());
            if item_path.file_name().unwrap() == "Cargo.toml.removeextension" {
                let item_parent_path = item_path.parent().unwrap();
                to = project_path.join(item_parent_path).join("Cargo.toml");
            }

            let file_contents =
                str::from_utf8(file.data.as_ref()).map_err(Error::ConvertBytesToString)?;

            if let Some(file_name) = to.file_name() {
                if file_name.to_str().unwrap_or("").contains("Cargo.toml") {
                    Self::write(
                        &to,
                        file_contents
                            .replace("contract-template", &self.args.name)
                            .as_str(),
                    )?;
                    continue;
                }
            }

            Self::write(&to, file_contents)?;
        }
        Ok(())
    }

    fn create_dir_all(path: &Path) -> Result<(), Error> {
        create_dir_all(path).map_err(|e| Error::Io(format!("creating directory: {path:?}"), e))
    }

    fn write(path: &Path, contents: &str) -> Result<(), Error> {
        write(path, contents).map_err(|e| Error::Io(format!("writing file: {path:?}"), e))
    }
}

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;

    use tempfile::TempDir;

    use super::*;

    const TEST_PROJECT_NAME: &str = "test-project";

    // Runs init command and checks that project has correct structure
    fn run_init(temp_dir: &TempDir, name: &str) {
        let project_dir = temp_dir.path().join(TEST_PROJECT_NAME);
        let runner = Runner {
            args: Cmd {
                project_path: project_dir.to_string_lossy().to_string(),
                name: name.to_string(),
                with_example: None,
                frontend_template: None,
                overwrite: None,
            },
            print: print::Print::new(false),
        };
        runner.run().unwrap();

        let expected_paths = ["contracts", "Cargo.toml", "README.md"];
        for path in &expected_paths {
            assert!(project_dir.join(path).exists());
        }

        let contract_dir = project_dir.join("contracts").join(name);

        assert!(contract_dir.exists());
        assert!(contract_dir.as_path().join("Cargo.toml").exists());
        assert!(contract_dir.as_path().join("src").join("lib.rs").exists());
        assert!(contract_dir.as_path().join("src").join("test.rs").exists());

        let contract_dir = project_dir.join("contracts").join(name);
        let cargo_toml_path = contract_dir.as_path().join("Cargo.toml");
        let cargo_toml_str = read_to_string(cargo_toml_path.clone()).unwrap();
        let doc = cargo_toml_str.parse::<toml_edit::Document>().unwrap();
        assert!(
            doc.get("dependencies")
                .unwrap()
                .get("soroban-sdk")
                .unwrap()
                .get("workspace")
                .unwrap()
                .as_bool()
                .unwrap(),
            "expected [dependencies.soroban-sdk] to be a workspace dependency"
        );
        assert!(
            doc.get("dev-dependencies")
                .unwrap()
                .get("soroban-sdk")
                .unwrap()
                .get("workspace")
                .unwrap()
                .as_bool()
                .unwrap(),
            "expected [dev-dependencies.soroban-sdk] to be a workspace dependency"
        );
        assert_ne!(
            0,
            doc.get("dev-dependencies")
                .unwrap()
                .get("soroban-sdk")
                .unwrap()
                .get("features")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            "expected [dev-dependencies.soroban-sdk] to have a features list"
        );
        assert!(
            doc.get("dev_dependencies").is_none(),
            "erroneous 'dev_dependencies' section"
        );
        assert_eq!(
            doc.get("lib")
                .unwrap()
                .get("crate-type")
                .unwrap()
                .as_array()
                .unwrap()
                .get(0)
                .unwrap()
                .as_str()
                .unwrap(),
            "cdylib",
            "expected [lib.crate-type] to be 'cdylib'"
        );
    }

    #[test]
    fn test_init() {
        let temp_dir = tempfile::tempdir().unwrap();

        run_init(&temp_dir, "hello_world");

        temp_dir.close().unwrap();
    }

    #[test]
    fn test_add() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Running init twice should add new member in the workspace
        run_init(&temp_dir, "hello_world");
        run_init(&temp_dir, "hello_world_2");

        temp_dir.close().unwrap();
    }
}
