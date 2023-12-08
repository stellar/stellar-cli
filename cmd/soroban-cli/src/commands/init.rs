use std::path::Path;
use std::{fs, io};

use clap::Parser;
use std::num::NonZeroU32;
use std::sync::atomic::AtomicBool;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    pub project_path: String,
}
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to create the project directory: {0}")]
    CreateDirError(#[from] io::Error),

    #[error("Failed to clone the template repository: {0}")]
    CloneError(#[from] gix::clone::Error),

    #[error("Failed to fetch the template repository: {0}")]
    FetchError(#[from] gix::clone::fetch::Error),

    #[error("Failed to checkout the template repository: {0}")]
    CheckoutError(#[from] gix::clone::checkout::main_worktree::Error),
}

const TEMPLATE_URL: &str = "https://github.com/AhaLabs/soroban-tutorial-project.git";

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) -> Result<(), Error> {
        println!("Creating a new soroban project at {}", self.project_path);
        let project_path = Path::new(&self.project_path);

        init(project_path, TEMPLATE_URL)
    }
}

fn init(project_path: &Path, template_url: &str) -> Result<(), Error> {
    let temp_dir = tempfile::tempdir()?;
    clone_repo(template_url, temp_dir.path())?;
    std::fs::create_dir_all(project_path)?;
    copy_contents(temp_dir.path(), project_path)?;
    Ok(())
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
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let entry_name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        let file_name = path.file_name().unwrap();
        let new_path = to.join(file_name);
        if path.is_dir() {
            if entry_name == ".git" {
                continue;
            }
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
        init(project_dir.as_path(), TEMPLATE_URL).unwrap();

        assert!(project_dir.as_path().join("README.md").exists());
        temp_dir.close().unwrap()
    }
}
