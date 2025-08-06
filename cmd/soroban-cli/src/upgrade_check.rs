use crate::config::upgrade_check::UpgradeCheck;
use crate::print::Print;
use crate::utils::http;
use semver::Version;
use serde::Deserialize;
use std::error::Error;
use std::io::IsTerminal;
use std::time::Duration;

const MINIMUM_CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60 * 24); // 1 day
const CRATES_IO_API_URL: &str = "https://crates.io/api/v1/crates/";
const NO_UPDATE_CHECK_ENV_VAR: &str = "STELLAR_NO_UPDATE_CHECK";

#[derive(Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    crate_: Crate,
}

#[derive(Deserialize)]
struct Crate {
    #[serde(rename = "max_stable_version")]
    max_stable_version: Version,
    #[serde(rename = "max_version")]
    max_version: Version, // This is the latest version, including pre-releases
}

/// Fetch the latest stable version of the crate from crates.io
async fn fetch_latest_crate_info() -> Result<Crate, Box<dyn Error>> {
    let crate_name = env!("CARGO_PKG_NAME");
    let url = format!("{CRATES_IO_API_URL}{crate_name}");
    let resp = http::client()
        .get(url)
        .send()
        .await?
        .json::<CrateResponse>()
        .await?;
    Ok(resp.crate_)
}

/// Print a warning if a new version of the CLI is available
pub async fn upgrade_check(quiet: bool) {
    // We should skip the upgrade check if we're not in a tty environment.
    if !std::io::stderr().is_terminal() {
        return;
    }

    // We should skip the upgrade check if the user has disabled it by setting
    // the environment variable (STELLAR_NO_UPDATE_CHECK)
    if std::env::var(NO_UPDATE_CHECK_ENV_VAR).is_ok() {
        return;
    }

    tracing::debug!("start upgrade check");

    let current_version = crate::commands::version::pkg();

    let mut stats = UpgradeCheck::load().unwrap_or_else(|e| {
        tracing::debug!("Failed to load upgrade check data: {e}");
        UpgradeCheck::default()
    });

    let now = chrono::Utc::now();
    // Skip fetch from crates.io if we've checked recently
    if now - MINIMUM_CHECK_INTERVAL >= stats.latest_check_time {
        match fetch_latest_crate_info().await {
            Ok(c) => {
                stats = UpgradeCheck {
                    latest_check_time: now,
                    max_stable_version: c.max_stable_version,
                    max_version: c.max_version,
                };
            }
            Err(e) => {
                tracing::debug!("Failed to fetch stellar-cli info from crates.io: {e}");
                // Only update the latest check time if the fetch failed
                // This way we don't spam the user with errors
                stats.latest_check_time = now;
            }
        }

        if let Err(e) = stats.save() {
            tracing::debug!("Failed to save upgrade check data: {e}");
        }
    }

    let current_version = Version::parse(current_version).unwrap();
    let latest_version = get_latest_version(&current_version, &stats);

    if current_version < *latest_version {
        let printer = Print::new(quiet);
        printer.warnln(format!(
            "A new release of stellar-cli is available: {current_version} -> {latest_version}"
        ));
    }

    tracing::debug!("finished upgrade check");
}

fn get_latest_version<'a>(current_version: &Version, stats: &'a UpgradeCheck) -> &'a Version {
    if current_version.pre.is_empty() {
        // If we are currently using a non-preview version
        &stats.max_stable_version
    } else {
        // If we are currently using a preview version
        if stats.max_stable_version > *current_version {
            // If there is a new stable version available, we should use that instead
            &stats.max_stable_version
        } else {
            &stats.max_version
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_latest_stable_version() {
        let _ = fetch_latest_crate_info().await.unwrap();
    }

    #[test]
    fn test_get_latest_version() {
        let stats = UpgradeCheck {
            latest_check_time: chrono::Utc::now(),
            max_stable_version: Version::parse("1.0.0").unwrap(),
            max_version: Version::parse("1.1.0-rc.1").unwrap(),
        };

        // When using a non-preview version
        let current_version = Version::parse("0.9.0").unwrap();
        let latest_version = get_latest_version(&current_version, &stats);
        assert_eq!(*latest_version, Version::parse("1.0.0").unwrap());

        // When using a preview version and a new stable version is available
        let current_version = Version::parse("0.9.0-rc.1").unwrap();
        let latest_version = get_latest_version(&current_version, &stats);
        assert_eq!(*latest_version, Version::parse("1.0.0").unwrap());

        // When using a preview version and no new stable version is available
        let current_version = Version::parse("1.1.0-beta.1").unwrap();
        let latest_version = get_latest_version(&current_version, &stats);
        assert_eq!(*latest_version, Version::parse("1.1.0-rc.1").unwrap());
    }

    #[test]
    fn test_semver_compare() {
        assert!(Version::parse("0.1.0").unwrap() < Version::parse("0.2.0").unwrap());
        assert!(Version::parse("0.1.0").unwrap() < Version::parse("0.1.1").unwrap());
        assert!(Version::parse("0.1.0").unwrap() > Version::parse("0.1.0-rc.1").unwrap());
        assert!(Version::parse("0.1.1-rc.1").unwrap() > Version::parse("0.1.0").unwrap());
        assert!(Version::parse("0.1.0-rc.2").unwrap() > Version::parse("0.1.0-rc.1").unwrap());
        assert!(Version::parse("0.1.0-rc.2").unwrap() > Version::parse("0.1.0-beta.2").unwrap());
        assert!(Version::parse("0.1.0-beta.2").unwrap() > Version::parse("0.1.0-alpha.2").unwrap());
        assert_eq!(
            Version::parse("0.1.0-beta.2").unwrap(),
            Version::parse("0.1.0-beta.2").unwrap()
        );
    }
}
