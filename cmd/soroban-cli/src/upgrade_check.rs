use crate::config::upgrade_check::UpgradeCheck;
use crate::print::Print;
use crate::utils::http;
use semver::Version;
use serde::Deserialize;
use std::error::Error;
use std::io::IsTerminal;
use std::time::Duration;

const MINIMUM_CHECK_INTERVAL: Duration = Duration::from_hours(24); // 1 day

// The shared HTTP client only bounds how long connecting may take, so a server
// that accepts the connection and then stalls would leave the request hanging
// indefinitely. Bound the whole request: this is a background nicety, and it
// must not be able to outlive the command it is running alongside.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
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

/// The path of the executable that is running, if it can be resolved.
///
/// `current_version` comes from `env!("CARGO_PKG_VERSION")`, so it describes
/// the binary that is running and nothing else. When more than one Stellar CLI
/// is installed (a stale `soroban` alongside a current `stellar`, or a Homebrew
/// install shadowed by a `cargo install` one), an old binary reports its own
/// old version and the message reads as though it were about the CLI the user
/// thinks they are running. Naming the executable makes the warning say which
/// install it is actually about.
///
/// Left as the process was started from, not canonicalized. Resolving symlinks
/// would name a path that moves under a normal in-place upgrade wherever a
/// package manager installs through one: Homebrew keeps
/// `/opt/homebrew/bin/stellar` pointing into a versioned Cellar directory and
/// retargets it on upgrade, so the canonical path changes while the install
/// does not. The path the user invokes outlives the file it happens to point
/// at, which is what makes it the install's identity.
///
/// It is also the more useful thing to name in a warning: it is the path the
/// user can act on, rather than one they never typed.
///
/// This only reaches as far as the platform allows. Linux resolves
/// `current_exe` through `/proc/self/exe` before we see it, so a retargeted
/// symlink still reads as a new install there; nothing in-process can recover
/// the invoked path once the kernel has resolved it. Not canonicalizing keeps
/// the platforms that do hand us the invoked path from losing it too.
pub fn running_binary() -> Option<String> {
    let path = std::env::current_exe().ok()?;

    Some(path.to_string_lossy().into_owned())
}

/// Fetch the latest stable version of the crate from crates.io
async fn fetch_latest_crate_info() -> Result<Crate, Box<dyn Error>> {
    let crate_name = env!("CARGO_PKG_NAME");
    let url = format!("{CRATES_IO_API_URL}{crate_name}");
    let resp = http::client()
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await?
        .json::<CrateResponse>()
        .await?;
    Ok(resp.crate_)
}

/// The upgrade warning, naming the executable it refers to when that can be
/// resolved.
pub fn upgrade_message(current_version: &Version, latest_version: &Version) -> String {
    let message =
        format!("A new release of Stellar CLI is available: {current_version} -> {latest_version}");

    match running_binary() {
        Some(binary) => format!("{message} ({binary})"),
        None => message,
    }
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

    if let Ok((true, current_version, latest_version)) = has_available_upgrade(true).await {
        let printer = Print::new(quiet);
        printer.warnln(upgrade_message(&current_version, &latest_version));
    }

    tracing::debug!("finished upgrade check");
}

pub async fn has_available_upgrade(
    cache: bool,
) -> Result<(bool, Version, Version), Box<dyn Error>> {
    let current_version = crate::commands::version::pkg();

    let mut stats = UpgradeCheck::load().unwrap_or_else(|e| {
        tracing::debug!("Failed to load upgrade check data: {e}");
        UpgradeCheck::default()
    });

    let now = chrono::Utc::now();
    // Skip fetch from crates.io if we've checked recently
    if !cache || now - MINIMUM_CHECK_INTERVAL >= stats.latest_check_time {
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

    Ok((
        *latest_version > current_version,
        current_version,
        latest_version.clone(),
    ))
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
    fn test_upgrade_message_names_the_running_binary() {
        let current = Version::parse("22.1.0").unwrap();
        let latest = Version::parse("23.3.0").unwrap();
        let binary = running_binary().expect("test binary path should resolve");

        let message = upgrade_message(&current, &latest);

        assert!(
            message.starts_with("A new release of Stellar CLI is available: 22.1.0 -> 23.3.0"),
            "unexpected message: {message}"
        );
        // Without this, a stale install's warning is indistinguishable from the
        // current install's -- the confusion reported in #2464.
        assert!(
            message.contains(&binary),
            "message should name the running binary, got: {message}"
        );
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
