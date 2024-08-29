use crate::config::self_outdated_check::SelfOutdatedCheck;
use crate::print::Print;
use semver::Version;
use serde::Deserialize;
use std::error::Error;
use std::time::Duration;

const MINIMUM_CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60 * 24); // 1 day
const CRATES_IO_API_URL: &str = "https://crates.io/api/v1/crates/";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    crate_: Crate,
}

#[derive(Deserialize)]
struct Crate {
    #[serde(rename = "max_stable_version")]
    max_stable_version: String,
}

/// Fetch the latest stable version of the crate from crates.io
fn fetch_latest_stable_version() -> Result<String, Box<dyn Error>> {
    let crate_name = crate::commands::version::pkg_name();
    let url = format!("{CRATES_IO_API_URL}{crate_name}");
    let response = ureq::get(&url).timeout(REQUEST_TIMEOUT).call()?;
    let crate_data: CrateResponse = response.into_json()?;
    Ok(crate_data.crate_.max_stable_version)
}

/// Print a warning if a new version of the CLI is available
pub fn print_upgrade_prompt(quiet: bool) {
    let current_version = crate::commands::version::pkg_version();
    let print = Print::new(quiet);

    let mut stats = SelfOutdatedCheck::load().unwrap_or_default();

    #[allow(clippy::cast_sign_loss)]
    let now = chrono::Utc::now().timestamp() as u64;

    // Skip fetch from crates.io if we've checked recently
    if now - stats.latest_check_time >= MINIMUM_CHECK_INTERVAL.as_secs() {
        if let Ok(latest_stable_version) = fetch_latest_stable_version() {
            stats = SelfOutdatedCheck {
                latest_check_time: now,
                latest_version: latest_stable_version,
            };
            stats.save().unwrap_or_default();
        }
    }

    let current_version = Version::parse(current_version).unwrap();
    let latest_version = Version::parse(&stats.latest_version).unwrap();

    if latest_version > current_version {
        print.println("");
        print.warnln(format!(
            "A new release of stellar-cli is available: {current_version} -> {latest_version}",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_latest_stable_version() {
        let version = fetch_latest_stable_version().unwrap();
        Version::parse(&version).unwrap();
    }
}
