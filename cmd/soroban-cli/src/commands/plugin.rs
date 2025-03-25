use std::{env, path::PathBuf, process::Command};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Plugin error: {0}")]
    Plugin(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn list() -> Result<Vec<String>, Error> {
    let path = env::var_os("PATH").ok_or_else(|| Error::Plugin("PATH not set".to_string()))?;
    let mut plugins = Vec::new();
    
    for dir in env::split_paths(&path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("soroban-") && is_executable(&path) {
                        plugins.push(name.to_string());
                    }
                }
            }
        }
    }
    
    Ok(plugins)
}

pub fn run() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return Err(Error::Plugin("No plugin specified".to_string()));
    }
    
    let plugin_name = format!("soroban-{}", args[1]);
    let status = Command::new(&plugin_name)
        .args(&args[2..])
        .status()
        .map_err(|e| Error::Plugin(format!("Failed to execute {}: {}", plugin_name, e)))?;
        
    if !status.success() {
        return Err(Error::Plugin(format!("{} failed", plugin_name)));
    }
    
    Ok(())
}

#[cfg(unix)]
fn is_executable(path: &PathBuf) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(path: &PathBuf) -> bool {
    path.extension()
        .map(|ext| ext.to_ascii_lowercase() == "exe")
        .unwrap_or(false)
}
