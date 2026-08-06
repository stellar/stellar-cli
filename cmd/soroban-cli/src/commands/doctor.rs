use clap::Parser;
use rustc_version::version;
use semver::Version;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{
    commands::{container::shared::Engine, global},
    config::{
        self, data,
        locator::{self, KeyType},
        network::{Network, DEFAULTS as DEFAULT_NETWORKS},
        upgrade_check::{CheckWriter, UpgradeCheck},
    },
    print::Print,
    rpc,
    upgrade_check::{check_performed_by, has_available_upgrade, running_binary, upgrade_message},
    utils::url::redact_url,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Network(#[from] config::network::Error),

    #[error(transparent)]
    RpcClient(#[from] rpc::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Data(#[from] data::Error),
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(false);

        // Read this before `check_version`, which refreshes the cache and would
        // otherwise record this very run as the writer -- hiding the mismatch
        // the report exists to reveal.
        let previous_cache_writer = version_cache_writer();

        check_version(&print).await?;
        check_installs(&print);
        show_version_cache_writer(&print, previous_cache_writer.as_ref());
        check_rust_version(&print);
        check_wasm_target(&print);
        check_optional_features(&print);
        check_container_engine(&print);
        show_config_path(&print, &self.config_locator)?;
        show_data_path(&print)?;
        show_xdr_version(&print);
        inspect_networks(&print, &self.config_locator).await?;

        Ok(())
    }
}

fn show_config_path(print: &Print, config_locator: &locator::Args) -> Result<(), Error> {
    let global_path = config_locator.global_config_path()?;

    print.gearln(format!(
        "Config directory: {}",
        global_path.to_string_lossy()
    ));

    Ok(())
}

fn show_data_path(print: &Print) -> Result<(), Error> {
    let path = data::data_local_dir()?;

    print.dirln(format!("Data directory: {}", path.to_string_lossy()));

    Ok(())
}

fn show_xdr_version(print: &Print) {
    let xdr = stellar_xdr::VERSION;

    print.infoln(format!("XDR version: {}", xdr.xdr));
}

async fn print_network(
    default: bool,
    print: &Print,
    name: &str,
    network: &Network,
) -> Result<(), Error> {
    let client = network.rpc_client()?;
    let version_info = client.get_version_info().await?;

    let prefix = if default {
        "Default network"
    } else {
        "Network"
    };

    print.globeln(format!(
        "{prefix} {name:?} ({})",
        redact_url(&network.rpc_url)
    ));
    print.blankln(format!("protocol {}", version_info.protocol_version));
    print.blankln(format!("rpc {}", version_info.version));

    Ok(())
}

async fn inspect_networks(print: &Print, config_locator: &locator::Args) -> Result<(), Error> {
    let saved_networks = KeyType::Network.list_paths(&config_locator.local_and_global()?)?;
    let default_networks = DEFAULT_NETWORKS
        .into_iter()
        .map(|(name, network)| ((*name).to_string(), network.into()));

    for (name, network) in default_networks {
        // Skip default mainnet, because it has no default rpc url.
        if name == "mainnet" {
            continue;
        }

        if print_network(true, print, &name, &network).await.is_err() {
            print.warnln(format!(
                "Default network {name:?} ({}) is unreachable",
                redact_url(&network.rpc_url)
            ));
        }
    }

    for (name, _) in &saved_networks {
        if let Ok(network) = config_locator.read_network(name) {
            if print_network(false, print, name, &network).await.is_err() {
                print.warnln(format!(
                    "Network {name:?} ({}) is unreachable",
                    redact_url(&network.rpc_url)
                ));
            }
        }
    }

    Ok(())
}

async fn check_version(print: &Print) -> Result<(), Error> {
    if let Ok((upgrade_available, current_version, latest_version)) =
        has_available_upgrade(false).await
    {
        if upgrade_available {
            print.warnln(upgrade_message(&current_version, &latest_version));
        } else {
            print.checkln(format!(
                "You are using the latest version of Stellar CLI: {current_version}"
            ));
        }
    }

    Ok(())
}

/// Which CLI last checked for a new release and wrote the shared version cache,
/// if it recorded itself.
fn version_cache_writer() -> Option<CheckWriter> {
    UpgradeCheck::load().ok()?.last_checked_by
}

/// Report which CLI last checked for a new release and wrote the shared version
/// cache.
///
/// Every install writes the same file, so the entry that decides when the next
/// check happens -- and the versions any warning is built from -- may have come
/// from a different install than the one being run. When it did, say so plainly:
/// that mismatch is the signal worth surfacing.
///
/// "Checked", not "refreshed": a check whose fetch failed still stamps the file
/// while leaving the recorded versions untouched, so the writer names the
/// install that paced the next check and cannot be said to have supplied the
/// versions stored beside it.
///
/// Identity is the executable path, not the recorded version. An in-place
/// upgrade leaves an older version recorded against the very path now running,
/// and that is one install, not two -- treating it as a mismatch would warn
/// about every upgrade until the cache was next written.
fn show_version_cache_writer(print: &Print, writer: Option<&CheckWriter>) {
    let Some(writer) = writer else {
        // Either no cache yet or one written before the CLI recorded this, so
        // there is nothing to report rather than something being wrong.
        print.infoln("Version cache was last checked by an unknown Stellar CLI".to_string());
        return;
    };

    let this_cli = check_performed_by();

    match (&writer.executable, &this_cli.executable) {
        (Some(written_by), Some(running)) if written_by == running => {
            if writer.version == this_cli.version {
                print.checkln(format!("Version cache last checked by: {writer}"));
            } else {
                // Same install, earlier version: the ordinary state after an
                // upgrade, and it corrects itself at the next check.
                let recorded = writer.version.as_deref().unwrap_or("an unknown version");
                let running = this_cli.version.as_deref().unwrap_or("unknown");

                print.infoln(format!(
                    "Version cache was last checked by this install running {recorded}; \
                     it is now {running}"
                ));
            }
        }
        (Some(_), Some(_)) => {
            print.warnln(format!(
                "Version cache was last checked by a different Stellar CLI: {writer}"
            ));
            print.blankln(format!("this one is {this_cli}"));
        }
        // Without both paths there is no identity to compare, so report the
        // writer without claiming it was or was not this install.
        _ => print.infoln(format!("Version cache was last checked by: {writer}")),
    }
}

/// The binary names this CLI ships under. Both are built from the same crate,
/// so a stale `soroban` runs the same upgrade check as a current `stellar` and
/// reports its own, older version.
const CLI_BINARY_NAMES: [&str; 2] = ["stellar", "soroban"];

/// Report the running executable and every other Stellar CLI on `PATH`.
///
/// Several executables at *different versions* is the case that makes the
/// upgrade warning look wrong: an old binary correctly reports its own old
/// version, but the user compares it against whichever binary
/// `stellar --version` resolves to and sees a contradiction. Listing them makes
/// that visible.
///
/// Count alone is not the signal. This crate ships two binaries, `stellar` and
/// `soroban`, so a single healthy install puts two files on `PATH` -- warning
/// about that would flag nearly every install. Warn when the versions actually
/// disagree.
fn check_installs(print: &Print) {
    match running_binary() {
        Some(binary) => print.infoln(format!("Running executable: {binary}")),
        None => print.warnln("Could not determine the running executable".to_string()),
    }

    let installs = find_installs();

    match (installs.len(), summarize_versions(&installs)) {
        (0, _) => print.warnln(
            "No Stellar CLI found on PATH; the running executable is not reachable by name"
                .to_string(),
        ),
        // One file, so nothing can disagree with it. Still list it: the running
        // executable is not necessarily the one `PATH` resolves by name, and the
        // line above carries no version.
        (1, _) => print.checkln("Only one Stellar CLI found on PATH:".to_string()),
        (count, InstalledVersions::Agreed(version)) => print.checkln(format!(
            "{count} Stellar CLI executables on PATH, all reporting {version}:"
        )),
        (count, InstalledVersions::Disagree { unanswered: 0 }) => print.warnln(format!(
            "Found {count} Stellar CLI executables on PATH reporting different versions; \
             an outdated one can report a version that disagrees with `stellar --version`:"
        )),
        // Only the ones that answered were seen to disagree. Folding the silent
        // ones into that count would attribute an observation to executables
        // that were never heard from.
        (count, InstalledVersions::Disagree { unanswered }) => print.warnln(format!(
            "Found {count} Stellar CLI executables on PATH; the {} that reported a version do \
             not agree ({unanswered} could not be asked); an outdated one can report a version \
             that disagrees with `stellar --version`:",
            count - unanswered
        )),
        // Not a version disagreement -- we could not establish one either way,
        // and saying "different versions" here would name a cause that has not
        // been observed. What the executables that did answer settled between
        // themselves still holds, so lead with it.
        (
            count,
            InstalledVersions::Unanswered {
                agreed: Some(version),
                unanswered,
            },
        ) => print.warnln(format!(
            "Found {count} Stellar CLI executables on PATH; every one that answered reports \
             {version}, but {unanswered} could not be asked, so a differing version cannot be \
             ruled out:"
        )),
        (count, InstalledVersions::Unanswered { agreed: None, .. }) => print.warnln(format!(
            "Found {count} Stellar CLI executables on PATH; none of them reported a version, \
             so whether they agree could not be determined:"
        )),
    }

    list_installs(print, &installs);
}

/// What the discovered executables establish about which version is in play.
#[derive(Debug, PartialEq, Eq)]
enum InstalledVersions {
    /// Every executable answered, and answered the same: one version is in play
    /// however many files carry it.
    Agreed(String),
    /// Two executables reported different versions. This is the case that makes
    /// the upgrade warning look wrong. Carries how many could not be asked: they
    /// took no part in the disagreement, so they are not part of its count.
    Disagree { unanswered: usize },
    /// At least one executable could not be asked for its version, and none of
    /// those that did contradict each other, so agreement is unestablished
    /// rather than absent. Carries the version the ones that answered settled
    /// on, if any answered at all, and how many did not.
    Unanswered {
        agreed: Option<String>,
        unanswered: usize,
    },
}

/// Classify the reported versions, keeping "they disagree" apart from "one of
/// them could not be asked".
///
/// A disagreement between known versions is reported even when other probes
/// failed: it is an observed fact, and the failures only add to it. Either way
/// the failures are counted separately, so neither outcome speaks for an
/// executable that was never heard from.
fn summarize_versions(installs: &[(PathBuf, Option<String>)]) -> InstalledVersions {
    let unanswered = installs
        .iter()
        .filter(|(_, version)| version.is_none())
        .count();

    let mut known = installs
        .iter()
        .filter_map(|(_, version)| version.as_deref());
    let first = known.next();

    if let Some(first) = first {
        if !known.all(|version| version == first) {
            return InstalledVersions::Disagree { unanswered };
        }
    }

    match (first, unanswered) {
        (Some(version), 0) => InstalledVersions::Agreed(version.to_string()),
        // Everything that answered agreed; `first` carries that version so the
        // one fact established here is not thrown away with the failed probes.
        _ => InstalledVersions::Unanswered {
            agreed: first.map(ToString::to_string),
            unanswered,
        },
    }
}

/// Every distinct Stellar CLI executable reachable by name on `PATH`, with the
/// version it reports.
fn find_installs() -> Vec<(PathBuf, Option<String>)> {
    let mut installs: Vec<(PathBuf, Option<String>)> = Vec::new();

    for name in CLI_BINARY_NAMES {
        let Ok(paths) = which::which_all(name) else {
            continue;
        };

        for path in paths {
            // `which_all` yields one entry per matching `PATH` element, so the
            // same binary shows up repeatedly when `PATH` has duplicates.
            let key = path.canonicalize().unwrap_or_else(|_| path.clone());
            if installs.iter().any(|(seen, _)| *seen == key) {
                continue;
            }

            let version = installed_version(&path);
            installs.push((key, version));
        }
    }

    installs
}

fn list_installs(print: &Print, installs: &[(PathBuf, Option<String>)]) {
    for (path, version) in installs {
        let version = version.as_deref().unwrap_or("unknown version");
        print.blankln(format!("- {} ({version})", path.to_string_lossy()));
    }
}

/// Ask a CLI executable for its version. Returns `None` if it cannot be run or
/// does not answer like a Stellar CLI.
fn installed_version(path: &Path) -> Option<String> {
    // `--only-version` predates neither every release nor every binary name:
    // releases old enough to cause the version confusion this check exists to
    // surface reject the flag, so fall back to parsing the full version banner.
    run_version(path, &["version", "--only-version"])
        .and_then(|output| parse_only_version(&output))
        .or_else(|| run_version(path, &["--version"]).and_then(|o| parse_version_banner(&o)))
}

fn run_version(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(path).args(args).output().ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
}

fn parse_only_version(output: &str) -> Option<String> {
    let version = output.trim();

    // An older CLI treats `--only-version` as an unknown argument and may still
    // exit zero while printing usage, so require a bare version.
    Version::parse(version).ok().map(|_| version.to_string())
}

/// Pull the version out of a banner like `stellar 22.8.0 (18f54cd...)`.
fn parse_version_banner(output: &str) -> Option<String> {
    output
        .lines()
        .next()?
        .split_whitespace()
        .find(|token| Version::parse(token).is_ok())
        .map(ToString::to_string)
}

fn check_rust_version(print: &Print) {
    match version() {
        Ok(rust_version) => {
            let v184 = Version::parse("1.84.0").unwrap();
            let v182 = Version::parse("1.82.0").unwrap();

            if rust_version >= v182 && rust_version < v184 {
                print.errorln(format!(
                    "Rust {rust_version} cannot be used to build contracts"
                ));
            } else {
                print.infoln(format!("Rust version: {rust_version}"));
            }
        }
        Err(_) => {
            print.warnln("Could not determine Rust version".to_string());
        }
    }
}

fn check_wasm_target(print: &Print) {
    let expected_target = get_expected_wasm_target();

    let Ok(output) = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
    else {
        print.warnln("Could not retrieve Rust targets".to_string());
        return;
    };

    if output.status.success() {
        let targets = String::from_utf8_lossy(&output.stdout);

        if targets.lines().any(|line| line.trim() == expected_target) {
            print.checkln(format!("Rust target `{expected_target}` is installed"));
        } else {
            print.errorln(format!("Rust target `{expected_target}` is not installed"));
        }
    } else {
        print.warnln("Could not retrieve Rust targets".to_string());
    }
}

fn check_container_engine(print: &Print) {
    // `resolved_default` silently falls back to docker on a bad
    // `STELLAR_CONTAINER_ENGINE`, so probe the raw value first to warn about a
    // typo instead of reporting a phantom docker as available.
    if !Engine::is_valid_engine() {
        let value = std::env::var("STELLAR_CONTAINER_ENGINE").unwrap_or_default();
        print.warnln(format!(
            "Unknown container engine `{value}`; expected one of: {}",
            Engine::supported_engines()
        ));
        return;
    }

    let engine = Engine::resolved_default();

    // `output()` succeeds as long as the binary spawned, regardless of exit
    // status, so it tells us whether the engine is installed on `PATH`.
    if Command::new(engine.program())
        .arg("--version")
        .output()
        .is_ok()
    {
        print.checkln(format!("Container engine `{engine}` is available"));
    } else {
        print.warnln(format!("Container engine `{engine}` is not installed"));
    }
}

fn check_optional_features(print: &Print) {
    #[cfg(feature = "additional-libs")]
    {
        print.checkln("Wasm optimization");
        print.checkln("Secure store (OS keyring)");
        print.checkln("Ledger hardware wallet");
    }

    #[cfg(not(feature = "additional-libs"))]
    {
        print.warnln(
            "The following features are disabled until `--features additional-libs` is used:",
        );
        print.blankln("- Wasm optimization");
        print.blankln("- Secure store (OS keyring)");
        print.blankln("- Ledger hardware wallet");
    }
}

fn get_expected_wasm_target() -> String {
    let Ok(current_version) = version() else {
        return "wasm32v1-none".into();
    };

    let v184 = Version::parse("1.84.0").unwrap();

    if current_version < v184 {
        "wasm32-unknown-unknown".into()
    } else {
        "wasm32v1-none".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_only_version_output() {
        assert_eq!(parse_only_version("27.1.0\n").as_deref(), Some("27.1.0"));
    }

    #[test]
    fn rejects_usage_text_printed_for_an_unknown_flag() {
        // A CLI old enough to reject `--only-version` must not be reported as
        // if the usage text were its version.
        let usage = "error: unexpected argument '--only-version' found\n\n\
                     Usage: soroban version [OPTIONS]\n";

        assert_eq!(parse_only_version(usage), None);
    }

    #[test]
    fn parses_version_from_an_older_cli_banner() {
        // Real output from a 22.8.0 `soroban`, which predates `--only-version`.
        let banner = "stellar 22.8.0 (18f54cdc0342726eb13ac14f84e899703d9f180a)\n\
                      stellar-xdr 22.1.0 (e139229708)\n\
                      xdr curr (529d5176f2)\n";

        assert_eq!(parse_version_banner(banner).as_deref(), Some("22.8.0"));
    }

    #[test]
    fn returns_none_for_a_banner_without_a_version() {
        assert_eq!(parse_version_banner("not a stellar cli\n"), None);
    }

    fn installs(entries: &[(&str, Option<&str>)]) -> Vec<(PathBuf, Option<String>)> {
        entries
            .iter()
            .map(|(path, version)| (PathBuf::from(path), version.map(ToString::to_string)))
            .collect()
    }

    #[test]
    fn agreement_needs_every_executable_to_have_answered() {
        assert_eq!(
            summarize_versions(&installs(&[
                ("/a/stellar", Some("27.1.0")),
                ("/a/soroban", Some("27.1.0")),
            ])),
            InstalledVersions::Agreed("27.1.0".to_string())
        );
    }

    #[test]
    fn distinct_known_versions_disagree() {
        assert_eq!(
            summarize_versions(&installs(&[
                ("/a/stellar", Some("27.1.0")),
                ("/b/soroban", Some("22.8.0")),
            ])),
            InstalledVersions::Disagree { unanswered: 0 }
        );
    }

    #[test]
    fn executables_that_cannot_be_asked_are_not_a_disagreement() {
        // Two unrunnable binaries tell us nothing about whether their versions
        // match, so reporting a disagreement would invent a cause.
        assert_eq!(
            summarize_versions(&installs(&[("/a/stellar", None), ("/b/soroban", None)])),
            InstalledVersions::Unanswered {
                agreed: None,
                unanswered: 2
            }
        );

        // One answered, the other did not: still nothing to contradict.
        assert_eq!(
            summarize_versions(&installs(&[
                ("/a/stellar", Some("27.1.0")),
                ("/b/soroban", None),
            ])),
            InstalledVersions::Unanswered {
                agreed: Some("27.1.0".to_string()),
                unanswered: 1
            }
        );
    }

    #[test]
    fn a_failed_probe_does_not_erase_the_agreement_around_it() {
        // Two agree and a third could not be asked. The agreement is the most
        // useful thing known about this machine, and it survives the probe that
        // failed beside it.
        assert_eq!(
            summarize_versions(&installs(&[
                ("/a/stellar", Some("27.1.0")),
                ("/a/soroban", Some("27.1.0")),
                ("/b/stellar", None),
            ])),
            InstalledVersions::Unanswered {
                agreed: Some("27.1.0".to_string()),
                unanswered: 1
            }
        );
    }

    #[test]
    fn an_observed_disagreement_outranks_a_failed_probe() {
        // The executable that never answered is counted apart from the two that
        // did: it is not one of the versions observed to differ.
        assert_eq!(
            summarize_versions(&installs(&[
                ("/a/stellar", Some("27.1.0")),
                ("/b/soroban", Some("22.8.0")),
                ("/c/stellar", None),
            ])),
            InstalledVersions::Disagree { unanswered: 1 }
        );
    }
}
