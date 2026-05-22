use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use clap::Parser;
use regex::Regex;
use soroban_spec_tools::contract::Spec;
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

use crate::{
    commands::{
        contract::build::verifiable::{
            bldimg_regex, source_repo_regex, source_rev_regex, tarball_sha256_regex,
            tarball_url_regex,
        },
        global,
    },
    config::{self, locator, network},
    print::Print,
    wasm,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract id or alias to fetch the WASM from the network.
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID", conflicts_with = "wasm")]
    pub contract_id: Option<config::UnresolvedContract>,

    /// Local WASM file to verify, instead of fetching from the network.
    #[arg(long)]
    pub wasm: Option<PathBuf>,

    /// Local tarball file or http(s) URL to use as the source when the WASM's
    /// recorded SEP-58 metadata has only `tarball_sha256` (no `tarball_url`).
    /// Accepts http(s) URLs or local file paths.
    #[arg(long)]
    pub tarball_url: Option<String>,

    /// Bypass interactive confirmation when the WASM's bldimg is not in the
    /// default trust list, or when the source is a tarball (tarballs are
    /// never default-trusted).
    #[arg(long)]
    pub trust: bool,

    #[command(flatten)]
    pub locator: locator::Args,

    #[command(flatten)]
    pub network: network::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("must pass exactly one of --id or --wasm")]
    MissingInput,

    #[error("reading wasm {0}: {1}")]
    ReadWasm(PathBuf, std::io::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Wasm(#[from] wasm::Error),

    #[error(transparent)]
    SpecTools(#[from] soroban_spec_tools::contract::Error),

    #[error("the WASM has no contractmetav0 custom section")]
    NoMeta,

    #[error("the WASM's contractmetav0 does not record a `bldimg` entry; cannot verify")]
    MissingBldimg,

    #[error("the WASM's contractmetav0 does not record any SEP-58 source-identification entry (source_repo+source_rev, tarball_url, or tarball_sha256); cannot verify")]
    MissingSourceId,

    #[error(
        "the WASM's `{field}` value {value:?} does not match the SEP-58 format regex `{regex}`"
    )]
    MetaFormat {
        field: &'static str,
        value: String,
        regex: &'static str,
    },

    #[error("the WASM records `source_rev` but not `source_repo`; SEP-58 requires both together")]
    SourceRevWithoutRepo,

    #[error("{kind} {value:?} is not in the default trust list, and stdin is not a terminal so we can't ask. Re-run with --trust to proceed.")]
    TrustRequired { kind: TrustKind, value: String },

    #[error("user declined to trust the {kind}; aborting")]
    TrustDeclined { kind: TrustKind },

    #[error("reading stdin: {0}")]
    Stdin(std::io::Error),
}

/// What kind of source is being trust-checked. Affects the default-trust
/// decision and shapes the prompt + error wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustKind {
    Bldimg,
    Tarball,
}

impl std::fmt::Display for TrustKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustKind::Bldimg => write!(f, "bldimg"),
            TrustKind::Tarball => write!(f, "tarball"),
        }
    }
}

/// Resolution of a single trust check before any I/O happens. Pure function of
/// the input — the run() side decides what to do with each variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustDecision {
    /// The value matches the default trust list for its kind. Proceed silently.
    Trusted,
    /// The value is not trusted by default, but `--trust` was passed. Proceed
    /// (and the caller may want to log).
    Overridden,
    /// Not trusted; the caller must prompt (TTY) or fail (non-TTY).
    NeedsConfirmation,
}

/// SEP-58 places no defaults on which images are trustworthy; we hardcode the
/// canonical `docker.io/stellar/stellar-cli` repo (digest-pinned) as the only
/// default-trusted image. Any other image — including mirrors and forks —
/// requires explicit confirmation.
const TRUSTED_BLDIMG_REGEX_STR: &str = r"^docker\.io/stellar/stellar-cli@sha256:[0-9a-f]{64}$";

fn trusted_bldimg_regex() -> Regex {
    Regex::new(TRUSTED_BLDIMG_REGEX_STR).unwrap()
}

/// Pure trust decision; no I/O. Tarball sources are never default-trusted.
pub fn trust_decision(value: &str, kind: TrustKind, trust_flag: bool) -> TrustDecision {
    let default_trusted = match kind {
        TrustKind::Bldimg => trusted_bldimg_regex().is_match(value),
        TrustKind::Tarball => false,
    };
    if default_trusted {
        TrustDecision::Trusted
    } else if trust_flag {
        TrustDecision::Overridden
    } else {
        TrustDecision::NeedsConfirmation
    }
}

/// SEP-58 metadata extracted from a contract's `contractmetav0` section.
///
/// `cliver` is intentionally not captured: the rebuild container re-injects it,
/// so verify's job is to ensure the rebuild's cliver matches the original's
/// (which it will when `bldimg` resolves to the same container).
#[derive(Debug, Clone)]
pub struct ExtractedMetadata {
    pub bldimg: String,
    pub source_repo: Option<String>,
    pub source_rev: Option<String>,
    pub tarball_url: Option<String>,
    pub tarball_sha256: Option<String>,
    pub bldopts: Vec<String>,
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(false);

        let wasm_bytes = self.fetch_wasm().await?;
        let meta = extract_metadata(&wasm_bytes)?;

        print.infoln(format!("bldimg: {}", meta.bldimg));
        if let Some(v) = &meta.source_repo {
            print.infoln(format!("source_repo: {v}"));
        }
        if let Some(v) = &meta.source_rev {
            print.infoln(format!("source_rev: {v}"));
        }
        if let Some(v) = &meta.tarball_url {
            print.infoln(format!("tarball_url: {v}"));
        }
        if let Some(v) = &meta.tarball_sha256 {
            print.infoln(format!("tarball_sha256: {v}"));
        }
        if !meta.bldopts.is_empty() {
            print.infoln(format!("bldopt entries ({}):", meta.bldopts.len()));
            for o in &meta.bldopts {
                print.blankln(format!("  • {o}"));
            }
        }

        // bldimg trust check is always required.
        require_trust(self.trust, TrustKind::Bldimg, &meta.bldimg, &print)?;

        // Tarball source: trust the URL we will actually fetch from (either the
        // value the WASM recorded, or the user's `--tarball-url` override).
        if let Some(url) = self.effective_tarball_url(&meta) {
            require_trust(self.trust, TrustKind::Tarball, &url, &print)?;
        }

        Ok(())
    }

    /// The tarball URL we'll actually retrieve from: the cli override if set,
    /// otherwise the value recorded in the WASM. Returns `None` for git-source
    /// builds (which aren't trust-checked here).
    fn effective_tarball_url(&self, meta: &ExtractedMetadata) -> Option<String> {
        self.tarball_url
            .clone()
            .or_else(|| meta.tarball_url.clone())
    }

    async fn fetch_wasm(&self) -> Result<Vec<u8>, Error> {
        match (&self.contract_id, &self.wasm) {
            (Some(id), None) => {
                let network = self.network.get(&self.locator)?;
                let resolved =
                    id.resolve_contract_id(&self.locator, &network.network_passphrase)?;
                Ok(wasm::fetch_from_contract(&resolved, &network).await?)
            }
            (None, Some(path)) => std::fs::read(path).map_err(|e| Error::ReadWasm(path.clone(), e)),
            _ => Err(Error::MissingInput),
        }
    }
}

/// Walk the WASM's `contractmetav0` entries and pull out the SEP-58 fields we
/// need to drive a rebuild. Errors when `bldimg` is absent or when no source
/// identification is recorded, since neither has a sensible default.
pub fn extract_metadata(wasm: &[u8]) -> Result<ExtractedMetadata, Error> {
    let spec = Spec::new(wasm)?;
    if spec.meta.is_empty() {
        return Err(Error::NoMeta);
    }

    let mut bldimg: Option<String> = None;
    let mut source_repo: Option<String> = None;
    let mut source_rev: Option<String> = None;
    let mut tarball_url: Option<String> = None;
    let mut tarball_sha256: Option<String> = None;
    let mut bldopts: Vec<String> = Vec::new();

    for entry in &spec.meta {
        let ScMetaEntry::ScMetaV0(ScMetaV0 { key, val }) = entry;
        let k = key.to_string();
        let v = val.to_string();
        match k.as_str() {
            "bldimg" => bldimg = Some(v),
            "source_repo" => source_repo = Some(v),
            "source_rev" => source_rev = Some(v),
            "tarball_url" => tarball_url = Some(v),
            "tarball_sha256" => tarball_sha256 = Some(v),
            "bldopt" => bldopts.push(v),
            _ => {} // cliver and any user --meta are intentionally ignored
        }
    }

    let bldimg = bldimg.ok_or(Error::MissingBldimg)?;
    if !bldimg_regex().is_match(&bldimg) {
        return Err(Error::MetaFormat {
            field: "bldimg",
            value: bldimg,
            regex: BLDIMG_REGEX_STR,
        });
    }

    if let Some(v) = &source_rev {
        if !source_rev_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "source_rev",
                value: v.clone(),
                regex: SOURCE_REV_REGEX_STR,
            });
        }
    }
    if let Some(v) = &source_repo {
        if !source_repo_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "source_repo",
                value: v.clone(),
                regex: SOURCE_REPO_REGEX_STR,
            });
        }
    }
    if let Some(v) = &tarball_url {
        if !tarball_url_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "tarball_url",
                value: v.clone(),
                regex: TARBALL_URL_REGEX_STR,
            });
        }
    }
    if let Some(v) = &tarball_sha256 {
        if !tarball_sha256_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "tarball_sha256",
                value: v.clone(),
                regex: TARBALL_SHA256_REGEX_STR,
            });
        }
    }

    // SEP-58 lists `source_repo+source_rev` as a conformant combination. We
    // refuse `source_rev` without `source_repo` here so the user sees a
    // pointed error rather than a downstream "can't clone repo" surprise.
    if source_rev.is_some() && source_repo.is_none() {
        return Err(Error::SourceRevWithoutRepo);
    }

    if source_repo.is_none()
        && source_rev.is_none()
        && tarball_url.is_none()
        && tarball_sha256.is_none()
    {
        return Err(Error::MissingSourceId);
    }

    Ok(ExtractedMetadata {
        bldimg,
        source_repo,
        source_rev,
        tarball_url,
        tarball_sha256,
        bldopts,
    })
}

/// Apply the trust decision: silent-OK, log-and-OK on override, or
/// prompt-vs-fail on `NeedsConfirmation` depending on whether stdin is a TTY.
fn require_trust(
    trust_flag: bool,
    kind: TrustKind,
    value: &str,
    print: &Print,
) -> Result<(), Error> {
    match trust_decision(value, kind, trust_flag) {
        TrustDecision::Trusted => Ok(()),
        TrustDecision::Overridden => {
            print.warnln(format!(
                "trusting {kind} {value} because --trust was passed"
            ));
            Ok(())
        }
        TrustDecision::NeedsConfirmation => {
            if !std::io::stdin().is_terminal() {
                return Err(Error::TrustRequired {
                    kind,
                    value: value.to_string(),
                });
            }
            confirm_interactively(kind, value)
        }
    }
}

fn confirm_interactively(kind: TrustKind, value: &str) -> Result<(), Error> {
    let prompt = match kind {
        TrustKind::Bldimg => format!(
            "Image {value} is not in the default trust list (only docker.io/stellar/stellar-cli is trusted by default)."
        ),
        TrustKind::Tarball => format!(
            "Tarball source {value} is not trusted by default. Tarballs always require confirmation."
        ),
    };
    eprintln!("{prompt}");
    eprint!("Trust this {kind} and continue? [y/N] ");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(Error::Stdin)?;
    if parse_yes(&line) {
        Ok(())
    } else {
        Err(Error::TrustDeclined { kind })
    }
}

/// Accepts y / Y / yes / YES / Yes (case-insensitive). Anything else, including
/// the empty string, is "no" — trust prompts default to declined.
pub fn parse_yes(answer: &str) -> bool {
    let a = answer.trim();
    a.eq_ignore_ascii_case("y") || a.eq_ignore_ascii_case("yes")
}

// These mirror the regex strings used in verifiable.rs. They're kept here only
// so `Error::MetaFormat` can render the regex back to the user as part of the
// error message. The actual matching uses the helpers from verifiable.rs.
const BLDIMG_REGEX_STR: &str =
    r"^(?:localhost(?::\d+)?|[^\s@/]*[.:][^\s@/]*)/[^\s@]+@sha256:[0-9a-f]{64}$";
const SOURCE_REV_REGEX_STR: &str = r"^[0-9a-f]{40}$";
const SOURCE_REPO_REGEX_STR: &str = r"^(https?://\S+|github:[^/\s]+/[^/\s]+)$";
const TARBALL_URL_REGEX_STR: &str = r"^https?://\S+$";
const TARBALL_SHA256_REGEX_STR: &str = r"^[0-9a-f]{64}$";

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use stellar_xdr::curr::{Limited, Limits, ScMetaEntry, ScMetaV0, WriteXdr};

    fn make_wasm_with_meta(entries: &[(&str, &str)]) -> Vec<u8> {
        let xdr = encode_meta(entries);
        let mut wasm = empty_wasm_module();
        wasm_gen::write_custom_section(&mut wasm, "contractmetav0", &xdr);
        wasm
    }

    fn empty_wasm_module() -> Vec<u8> {
        // Minimal valid WASM: magic + version, no sections.
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    fn encode_meta(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut writer = Limited::new(Cursor::new(&mut buf), Limits::none());
        for (k, v) in entries {
            ScMetaEntry::ScMetaV0(ScMetaV0 {
                key: (*k).to_string().try_into().unwrap(),
                val: (*v).to_string().try_into().unwrap(),
            })
            .write_xdr(&mut writer)
            .unwrap();
        }
        buf
    }

    fn good_bldimg() -> String {
        format!("docker.io/stellar/stellar-cli@sha256:{}", "a".repeat(64))
    }

    #[test]
    fn extract_metadata_happy_path_git_source() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", &"b".repeat(40)),
            ("bldopt", "--locked"),
            ("bldopt", "--meta=home_domain=fnando.com"),
            ("home_domain", "fnando.com"),
            ("cliver", "26.0.0#abcdef"),
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        assert_eq!(meta.bldimg, good_bldimg());
        assert_eq!(
            meta.source_repo.as_deref(),
            Some("https://github.com/foo/bar")
        );
        assert_eq!(meta.source_rev.as_deref(), Some("b".repeat(40).as_str()));
        assert_eq!(
            meta.bldopts,
            vec![
                "--locked".to_string(),
                "--meta=home_domain=fnando.com".to_string()
            ]
        );
        assert!(meta.tarball_url.is_none());
        assert!(meta.tarball_sha256.is_none());
    }

    #[test]
    fn extract_metadata_happy_path_tarball_pair() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("tarball_url", "https://example.com/src.tar.gz"),
            ("tarball_sha256", &"f".repeat(64)),
            ("bldopt", "--locked"),
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        assert_eq!(
            meta.tarball_url.as_deref(),
            Some("https://example.com/src.tar.gz")
        );
        assert_eq!(
            meta.tarball_sha256.as_deref(),
            Some("f".repeat(64).as_str())
        );
        assert!(meta.source_repo.is_none());
        assert!(meta.source_rev.is_none());
    }

    #[test]
    fn extract_metadata_missing_bldimg_errors() {
        let wasm = make_wasm_with_meta(&[
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", &"b".repeat(40)),
        ]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::MissingBldimg));
    }

    #[test]
    fn extract_metadata_missing_source_id_errors() {
        let wasm = make_wasm_with_meta(&[("bldimg", &good_bldimg())]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::MissingSourceId));
    }

    #[test]
    fn extract_metadata_source_rev_without_repo_errors() {
        let wasm =
            make_wasm_with_meta(&[("bldimg", &good_bldimg()), ("source_rev", &"b".repeat(40))]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::SourceRevWithoutRepo));
    }

    #[test]
    fn extract_metadata_bad_bldimg_format_errors() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", "stellar/stellar-cli@sha256:abc"), // implicit hub + short
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", &"b".repeat(40)),
        ]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(
            err,
            Error::MetaFormat {
                field: "bldimg",
                ..
            }
        ));
    }

    #[test]
    fn extract_metadata_bad_source_rev_format_errors() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", "not-a-sha"),
        ]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(
            err,
            Error::MetaFormat {
                field: "source_rev",
                ..
            }
        ));
    }

    #[test]
    fn extract_metadata_bad_tarball_sha256_format_errors() {
        let wasm = make_wasm_with_meta(&[("bldimg", &good_bldimg()), ("tarball_sha256", "abc")]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(
            err,
            Error::MetaFormat {
                field: "tarball_sha256",
                ..
            }
        ));
    }

    #[test]
    fn extract_metadata_ignores_cliver_and_user_meta() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", &"b".repeat(40)),
            ("cliver", "26.0.0#abcdef"),
            ("home_domain", "fnando.com"),
            ("author", "alice"),
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        // cliver and user meta land in neither bldopts nor source-ids.
        assert!(meta.bldopts.is_empty());
    }

    #[test]
    fn extract_metadata_empty_meta_errors() {
        let wasm = empty_wasm_module(); // no contractmetav0 section
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::NoMeta));
    }

    #[test]
    fn trust_decision_bldimg_canonical_is_trusted() {
        let img = format!("docker.io/stellar/stellar-cli@sha256:{}", "a".repeat(64));
        assert_eq!(
            trust_decision(&img, TrustKind::Bldimg, false),
            TrustDecision::Trusted
        );
        assert_eq!(
            trust_decision(&img, TrustKind::Bldimg, true),
            TrustDecision::Trusted
        );
    }

    #[test]
    fn trust_decision_bldimg_other_registry_needs_confirmation() {
        let img = format!("ghcr.io/stellar/stellar-cli@sha256:{}", "a".repeat(64));
        assert_eq!(
            trust_decision(&img, TrustKind::Bldimg, false),
            TrustDecision::NeedsConfirmation
        );
        assert_eq!(
            trust_decision(&img, TrustKind::Bldimg, true),
            TrustDecision::Overridden
        );
    }

    #[test]
    fn trust_decision_bldimg_other_repo_on_dockerhub_needs_confirmation() {
        // Same registry but different repo (fork) — not trusted.
        let img = format!("docker.io/fnando/stellar-cli@sha256:{}", "a".repeat(64));
        assert_eq!(
            trust_decision(&img, TrustKind::Bldimg, false),
            TrustDecision::NeedsConfirmation
        );
    }

    #[test]
    fn trust_decision_tarball_always_needs_confirmation() {
        assert_eq!(
            trust_decision(
                "https://github.com/foo/bar.tar.gz",
                TrustKind::Tarball,
                false
            ),
            TrustDecision::NeedsConfirmation
        );
        assert_eq!(
            trust_decision("/local/foo.tar.gz", TrustKind::Tarball, false),
            TrustDecision::NeedsConfirmation
        );
    }

    #[test]
    fn trust_decision_tarball_override_with_trust() {
        assert_eq!(
            trust_decision(
                "https://github.com/foo/bar.tar.gz",
                TrustKind::Tarball,
                true
            ),
            TrustDecision::Overridden
        );
    }

    #[test]
    fn parse_yes_accepts_all_case_variants() {
        for yes in ["y", "Y", "yes", "YES", "Yes", "yEs", " y ", "yes\n"] {
            assert!(parse_yes(yes), "{yes:?} should be yes");
        }
    }

    #[test]
    fn parse_yes_rejects_anything_else() {
        for no in ["", "n", "N", "no", "NO", "x", "yup", "yeah", " "] {
            assert!(!parse_yes(no), "{no:?} should not be yes");
        }
    }
}
