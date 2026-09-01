use crate::config::upgrade_check::UpgradeCheck;
use crate::print::Print;
use crate::utils::http;
use semver::Version;
use serde::Deserialize;
use std::error::Error;
use std::io::IsTerminal;
use std::time::Duration;

const MINIMUM_CHECK_INTERVAL: Duration = Duration::from_hours(24); // 1 day
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

    if let Ok((true, current_version, latest_version)) = has_available_upgrade(true).await {
        let printer = Print::new(quiet);
        printer.warnln(format!(
            "A new release of Stellar CLI is available: {current_version} -> {latest_version}"
        ));
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
        let fetched = apply_fetch_result(&mut stats, fetch_latest_crate_info().await, now);

        if let Err(e) = &fetched {
            tracing::debug!("Failed to fetch stellar-cli info from crates.io: {e}");
        }

        if let Err(e) = stats.save() {
            tracing::debug!("Failed to save upgrade check data: {e}");
        }

        // Without a fresh answer from crates.io the cached versions may be
        // arbitrarily old, and reporting them as "the latest release" tells
        // the user to upgrade to a version that is itself outdated (#2464).
        // The stale versions were reset above, so invocations inside the
        // throttle window that skip this block stay silent as well, and the
        // check time was still advanced, so the fetch is retried at most
        // once per interval rather than on every invocation.
        fetched?;
    }

    let current_version = Version::parse(current_version).unwrap();
    let latest_version = get_latest_version(&current_version, &stats);

    Ok((
        *latest_version > current_version,
        current_version,
        latest_version.clone(),
    ))
}

/// Folds the outcome of a crates.io fetch into the cached stats.
///
/// On success the cached versions and check time are replaced wholesale. On
/// failure the cached versions are reset to the `0.0.0` defaults — they are
/// stale, not authoritative, and leaving them in place would let the next
/// invocation inside the throttle window (which skips the fetch entirely)
/// print an upgrade warning from them (#2464) — while the check time still
/// advances so a failing fetch is retried at most once per check interval
/// instead of on every command. The error is propagated so callers know the
/// versions in `stats` do not reflect a current answer from crates.io.
fn apply_fetch_result(
    stats: &mut UpgradeCheck,
    result: Result<Crate, Box<dyn Error>>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), Box<dyn Error>> {
    match result {
        Ok(c) => {
            *stats = UpgradeCheck {
                latest_check_time: now,
                max_stable_version: c.max_stable_version,
                max_version: c.max_version,
            };
            Ok(())
        }
        Err(e) => {
            *stats = UpgradeCheck {
                latest_check_time: now,
                ..UpgradeCheck::default()
            };
            Err(e)
        }
    }
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
    fn test_apply_fetch_result_success_replaces_cached_versions() {
        let mut stats = UpgradeCheck {
            latest_check_time: chrono::DateTime::UNIX_EPOCH,
            max_stable_version: Version::parse("25.1.0").unwrap(),
            max_version: Version::parse("25.1.0").unwrap(),
        };
        let now = chrono::Utc::now();

        let result = apply_fetch_result(
            &mut stats,
            Ok(Crate {
                max_stable_version: Version::parse("25.2.0").unwrap(),
                max_version: Version::parse("26.0.0-rc.1").unwrap(),
            }),
            now,
        );

        assert!(result.is_ok());
        assert_eq!(stats.latest_check_time, now);
        assert_eq!(stats.max_stable_version, Version::parse("25.2.0").unwrap());
        assert_eq!(stats.max_version, Version::parse("26.0.0-rc.1").unwrap());
    }

    #[test]
    fn test_apply_fetch_result_failure_is_propagated_not_swallowed() {
        // Regression for #2464: when the crates.io fetch fails, the cached
        // versions may be arbitrarily stale. The failure must surface to the
        // caller so no upgrade warning is printed from stale data, and the
        // stale versions must be cleared so later invocations inside the
        // throttle window (which skip the fetch) cannot warn from them,
        // while the check time still advances to keep the retry throttled.
        let stale_time = chrono::Utc::now() - chrono::Duration::days(19);
        let mut stats = UpgradeCheck {
            latest_check_time: stale_time,
            max_stable_version: Version::parse("25.1.0").unwrap(),
            max_version: Version::parse("25.1.0").unwrap(),
        };
        let now = chrono::Utc::now();

        let result = apply_fetch_result(&mut stats, Err("network is down".into()), now);

        assert!(
            result.is_err(),
            "fetch failure must propagate to the caller"
        );
        assert_eq!(
            stats.latest_check_time, now,
            "check time must advance so the retry stays throttled"
        );
        assert_eq!(
            stats.max_stable_version,
            Version::new(0, 0, 0),
            "stale max_stable_version must be cleared so throttled invocations cannot warn from it"
        );
        assert_eq!(
            stats.max_version,
            Version::new(0, 0, 0),
            "stale max_version must be cleared so throttled invocations cannot warn from it"
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
