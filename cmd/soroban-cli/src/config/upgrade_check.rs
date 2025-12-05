use crate::config::locator;
use chrono::{DateTime, Utc};
use jsonrpsee_core::Serialize;
use semver::Version;
use serde::Deserialize;
use serde_json;
use std::{fs, sync::OnceLock};

const FILE_NAME: &str = "upgrade_check.json";

/// The `UpgradeCheck` struct represents the state of the upgrade check.
/// This state is global and stored in the `upgrade_check.json` file in
/// the global configuration directory.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct UpgradeCheck {
    /// The time of the latest check for a new version of the CLI.
    pub latest_check_time: DateTime<Utc>,
    /// The latest stable version of the CLI available on crates.io.
    pub max_stable_version: Version,
    /// The latest version of the CLI available on crates.io, including pre-releases.
    pub max_version: Version,
}

impl Default for UpgradeCheck {
    fn default() -> Self {
        Self {
            latest_check_time: DateTime::<Utc>::UNIX_EPOCH,
            max_stable_version: Version::new(0, 0, 0),
            max_version: Version::new(0, 0, 0),
        }
    }
}

impl UpgradeCheck {
    /// Loads the state of the upgrade check from the global configuration directory.
    /// If the file doesn't exist, returns a default instance of `UpgradeCheck`.
    pub fn load() -> Result<Self, locator::Error> {
        let locator = locator::Args {
            global: false,
            config_dir: None,
            cached_keys: OnceLock::new() 
        };
        let path = locator.global_config_path()?.join(FILE_NAME);
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read(&path)
            .map_err(|error| locator::Error::UpgradeCheckReadFailed { path, error })?;
        Ok(serde_json::from_slice(data.as_slice())?)
    }

    /// Saves the state of the upgrade check to the `upgrade_check.json` file in the global configuration directory.
    pub fn save(&self) -> Result<(), locator::Error> {
        let locator = locator::Args {
            global: false,
            config_dir: None,
            cached_keys: OnceLock::new() 
        };
        let path = locator.global_config_path()?.join(FILE_NAME);
        let path = locator::ensure_directory(path)?;
        let data = serde_json::to_string(self).map_err(|_| locator::Error::ConfigSerialization)?;
        fs::write(&path, data)
            .map_err(|error| locator::Error::UpgradeCheckWriteFailed { path, error })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_upgrade_check_load_save() {
        // Set the `XDG_CONFIG_HOME` environment variable to a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        // Test default loading
        let default_check = UpgradeCheck::load().unwrap();
        assert_eq!(default_check, UpgradeCheck::default());
        assert_eq!(
            default_check.latest_check_time,
            DateTime::<Utc>::from_timestamp_millis(0).unwrap()
        );
        assert_eq!(default_check.max_stable_version, Version::new(0, 0, 0));

        // Test saving and loading
        let saved_check = UpgradeCheck {
            latest_check_time: DateTime::<Utc>::from_timestamp(1_234_567_890, 0).unwrap(),
            max_stable_version: Version::new(1, 2, 3),
            max_version: Version::parse("1.2.4-rc.1").unwrap(),
        };
        saved_check.save().unwrap();
        let loaded_check = UpgradeCheck::load().unwrap();
        assert_eq!(loaded_check, saved_check);
    }
}
