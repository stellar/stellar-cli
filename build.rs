use serde_derive::{Deserialize, Serialize};
use std::{fs::read_to_string, process::Command, str};

fn main() {
    println!("cargo:rerun-if-changed=.");

    let mut git_sha: Option<String> = None;

    if let Ok(vcs_info) = read_to_string(".cargo_vcs_info.json") {
        let vcs_info: Result<CargoVcsInfo, _> = serde_json::from_str(&vcs_info);
        if let Ok(vcs_info) = vcs_info {
            git_sha = Some(vcs_info.git.sha1);
        }
    }

    if git_sha.is_none() {
        if let Ok(git_describe) = Command::new("git")
            .arg("describe")
            .arg("--always")
            .arg("--exclude='*'")
            .arg("--long")
            .arg("--dirty")
            .output()
            .map(|o| o.stdout)
        {
            git_sha = str::from_utf8(&git_describe).ok().map(str::to_string);
        }
    }

    if let Some(git_sha) = git_sha {
        println!("cargo:rustc-env=GIT_SHA={}", git_sha);
    }
}

#[derive(Serialize, Deserialize, Default)]
struct CargoVcsInfo {
    git: CargoVcsInfoGit,
}

#[derive(Serialize, Deserialize, Default)]
struct CargoVcsInfoGit {
    sha1: String,
}
