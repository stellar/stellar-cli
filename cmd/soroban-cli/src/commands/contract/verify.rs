use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use regex::Regex;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract::Spec;
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

use crate::{
    commands::{
        contract::build::verifiable::{
            bldimg_regex, source_uri_regex, source_sha256_regex
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

    /// Local source code file or http(s) URL to use as the source when the WASM's
    /// recorded SEP-58 metadata has only `source_sha256` (no `source_uri`).
    /// Accepts http(s) URLs or local file paths.
    #[arg(long)]
    pub source_uri: Option<String>,

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

    #[error("the WASM's contractmetav0 does not record a `source_sha256` entry; cannot verify")]
    MissingSourceSha256,

    #[error(
        "the WASM's `{field}` value {value:?} does not match the SEP-58 format regex `{regex}`"
    )]
    MetaFormat {
        field: &'static str,
        value: String,
        regex: &'static str,
    },

    #[error("{kind} {value:?} is not in the default trust list, and stdin is not a terminal so we can't ask. Re-run with --trust to proceed.")]
    TrustRequired { kind: TrustKind, value: String },

    #[error("user declined to trust the {kind}; aborting")]
    TrustDeclined { kind: TrustKind },

    #[error("reading stdin: {0}")]
    Stdin(std::io::Error),

    #[error("the WASM records only `source_sha256` (no `source_uri`). Pass `--source-uri URL_OR_PATH` to provide retrieval.")]
    SourceUriRequired,

    #[error("downloading {url}: {source}")]
    SourceDownload { url: String, source: reqwest::Error },

    #[error("reading local source code {path}: {source}")]
    SourceRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("source code sha256 mismatch: expected {expected}, got {actual}")]
    SourceHashMismatch { expected: String, actual: String },

    #[error("extracting source code into {path}: {source}")]
    SourceExtract {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("creating tempdir: {0}")]
    TempDir(std::io::Error),
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
    pub source_uri: Option<String>,
    pub source_sha256: Option<String>,
    pub bldopts: Vec<String>,
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(false);

        let wasm_bytes = self.fetch_wasm().await?;
        let meta = extract_metadata(&wasm_bytes)?;

        print.infoln(format!("bldimg: {}", meta.bldimg));

        if let Some(v) = &meta.source_uri {
            print.infoln(format!("source_uri: {v}"));
        }

        if let Some(v) = &meta.source_sha256 {
            print.infoln(format!("source_sha256: {v}"));
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
        // value the WASM recorded, or the user's `--source-uri` override).
        if let Some(url) = self.effective_source_uri(&meta) {
            require_trust(self.trust, TrustKind::Tarball, &url, &print)?;
        }

        // Materialize the recorded source into a tempdir so the next step
        // (the rebuild — to land in a follow-up commit) can bind-mount it.
        // The TempDir keeps the directory alive only for this scope; the
        // rebuild needs to happen before we return.
        let workdir = tempfile::TempDir::new().map_err(Error::TempDir)?;
        materialize_source(&meta, self.source_uri.as_deref(), workdir.path(), &print).await?;
        print.checkln(format!(
            "Source materialized at {}",
            workdir.path().display()
        ));

        Ok(())
    }

    /// The tarball URL we'll actually retrieve from: the cli override if set,
    /// otherwise the value recorded in the WASM. Returns `None` when neither
    /// records a `source_uri` (only `source_sha256` is set), in which case
    /// there's nothing to trust-check here.
    fn effective_source_uri(&self, meta: &ExtractedMetadata) -> Option<String> {
        self.source_uri
            .clone()
            .or_else(|| meta.source_uri.clone())
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
/// need to drive a rebuild. Errors when `bldimg` or `source_sha256` is absent,
/// since neither has a sensible default. `source_uri` is optional.
pub fn extract_metadata(wasm: &[u8]) -> Result<ExtractedMetadata, Error> {
    let spec = Spec::new(wasm)?;
    if spec.meta.is_empty() {
        return Err(Error::NoMeta);
    }

    let mut bldimg: Option<String> = None;
    let mut source_uri: Option<String> = None;
    let mut source_sha256: Option<String> = None;
    let mut bldopts: Vec<String> = Vec::new();

    for entry in &spec.meta {
        let ScMetaEntry::ScMetaV0(ScMetaV0 { key, val }) = entry;
        let k = key.to_string();
        let v = val.to_string();
        match k.as_str() {
            "bldimg" => bldimg = Some(v),
            "source_uri" => source_uri = Some(v),
            "source_sha256" => source_sha256 = Some(v),
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

    if let Some(v) = &source_uri {
        if !source_uri_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "source_uri",
                value: v.clone(),
                regex: SOURCE_URL_REGEX_STR,
            });
        }
    }
    if let Some(v) = &source_sha256 {
        if !source_sha256_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "source_sha256",
                value: v.clone(),
                regex: SOURCE_SHA256_REGEX_STR,
            });
        }
    }

    if source_sha256.is_none() {
        return Err(Error::MissingSourceSha256);
    }

    Ok(ExtractedMetadata {
        bldimg,
        source_uri,
        source_sha256,
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

/// Materialize the recorded source tree into `target`. Picks the path based on
/// what the WASM recorded:
///   - source_uri (with optional sha256) → download/read, optional sha-check,
///     extract via `tar`
///   - source_sha256 only → require `--source-uri` on the cli and use it as
///     the retrieval channel
///
/// `source_uri_override` is the cli's `--source-uri` flag value; when set, it
/// wins over whatever the WASM recorded, and may be an http(s) URL or a local
/// file path.
async fn materialize_source(
    meta: &ExtractedMetadata,
    source_uri_override: Option<&str>,
    target: &Path,
    print: &Print,
) -> Result<(), Error> {
    let tarball_source = source_uri_override
        .map(str::to_string)
        .or_else(|| meta.source_uri.clone());
    let Some(source) = tarball_source else {
        // No source_uri anywhere — only source_sha256 is set.
        return Err(Error::SourceUriRequired);
    };

    print.infoln(format!("Fetching source code from {source}"));
    let bytes = fetch_tarball_bytes(&source).await?;

    if let Some(expected) = &meta.source_sha256 {
        verify_source_sha256(&bytes, expected)?;
        print.checkln("source code sha256 matches");
    }
    extract_tarball(&bytes, target)?;
    Ok(())
}

/// Retrieve the tarball bytes. `source` is either an `http(s)://` URL or a
/// local file path. The split is by prefix, not by attempting both — keeps
/// behavior predictable.
async fn fetch_tarball_bytes(source: &str) -> Result<Vec<u8>, Error> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let resp = reqwest::get(source)
            .await
            .map_err(|e| Error::SourceDownload {
                url: source.to_string(),
                source: e,
            })?;
        let bytes = resp
            .error_for_status()
            .map_err(|e| Error::SourceDownload {
                url: source.to_string(),
                source: e,
            })?
            .bytes()
            .await
            .map_err(|e| Error::SourceDownload {
                url: source.to_string(),
                source: e,
            })?;
        Ok(bytes.to_vec())
    } else {
        std::fs::read(source).map_err(|e| Error::SourceRead {
            path: PathBuf::from(source),
            source: e,
        })
    }
}

fn verify_source_sha256(bytes: &[u8], expected: &str) -> Result<(), Error> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(Error::SourceHashMismatch {
            expected: expected.to_string(),
            actual,
        })
    }
}

fn extract_tarball(bytes: &[u8], target: &Path) -> Result<(), Error> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(target).map_err(|e| Error::SourceExtract {
        path: target.to_path_buf(),
        source: e,
    })
}

// These mirror the regex strings used in verifiable.rs. They're kept here only
// so `Error::MetaFormat` can render the regex back to the user as part of the
// error message. The actual matching uses the helpers from verifiable.rs.
const BLDIMG_REGEX_STR: &str =
    r"^(?:localhost(?::\d+)?|[^\s@/]*[.:][^\s@/]*)/[^\s@]+@sha256:[0-9a-f]{64}$";
const SOURCE_URL_REGEX_STR: &str = r"^https?://\S+$";
const SOURCE_SHA256_REGEX_STR: &str = r"^[0-9a-f]{64}$";

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
    fn extract_metadata_happy_path_tarball_pair() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("source_uri", "https://example.com/src.tar.gz"),
            ("source_sha256", &"f".repeat(64)),
            ("bldopt", "--locked"),
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        assert_eq!(
            meta.source_uri.as_deref(),
            Some("https://example.com/src.tar.gz")
        );
        assert_eq!(
            meta.source_sha256.as_deref(),
            Some("f".repeat(64).as_str())
        );
    }

    #[test]
    fn extract_metadata_missing_bldimg_errors() {
        let wasm = make_wasm_with_meta(&[("source_sha256", &"b".repeat(64))]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::MissingBldimg));
    }

    #[test]
    fn extract_metadata_missing_source_id_errors() {
        let wasm = make_wasm_with_meta(&[("bldimg", &good_bldimg())]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::MissingSourceSha256));
    }

    #[test]
    fn extract_metadata_bad_bldimg_format_errors() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", "stellar/stellar-cli@sha256:abc"), // implicit hub + short
            ("source_sha256", &"b".repeat(64)),
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
    fn extract_metadata_bad_source_sha256_format_errors() {
        let wasm = make_wasm_with_meta(&[("bldimg", &good_bldimg()), ("source_sha256", "abc")]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(
            err,
            Error::MetaFormat {
                field: "source_sha256",
                ..
            }
        ));
    }

    #[test]
    fn extract_metadata_ignores_cliver_and_user_meta() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("source_sha256", &"b".repeat(64)),
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

    #[test]
    fn verify_source_sha256_matches() {
        let bytes = b"hello, sep-58";
        let digest = format!("{:x}", Sha256::digest(bytes));
        verify_source_sha256(bytes, &digest).unwrap();
        // Case-insensitive: SEP-58 mandates lowercase but be lenient on input.
        verify_source_sha256(bytes, &digest.to_ascii_uppercase()).unwrap();
    }

    #[test]
    fn verify_source_sha256_mismatch_errors() {
        let bytes = b"hello, sep-58";
        let bogus = "0".repeat(64);
        let err = verify_source_sha256(bytes, &bogus).unwrap_err();
        assert!(matches!(err, Error::SourceHashMismatch { .. }));
    }

    /// Build a tiny in-memory tar.gz with a single file and confirm extraction
    /// drops the file at the expected path. Exercises the pure-Rust pipeline
    /// (no shelling out, so this passes on Windows too).
    #[test]
    fn extract_tarball_unpacks_into_target() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            let payload = b"contents";
            let mut header = tar::Header::new_gnu();
            header.set_path("hello.txt").unwrap();
            header.set_size(payload.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, &payload[..]).unwrap();
            builder.finish().unwrap();
        }

        let mut gz = Vec::new();
        {
            let mut enc = GzEncoder::new(&mut gz, Compression::default());
            enc.write_all(&tar_bytes).unwrap();
            enc.finish().unwrap();
        }

        let dir = tempfile::TempDir::new().unwrap();
        extract_tarball(&gz, dir.path()).unwrap();
        let extracted = std::fs::read(dir.path().join("hello.txt")).unwrap();
        assert_eq!(extracted, b"contents");
    }

    #[tokio::test]
    async fn materialize_source_errors_when_only_source_sha256() {
        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: None,
            source_sha256: Some("f".repeat(64)),
            bldopts: Vec::new(),
        };
        let dir = tempfile::TempDir::new().unwrap();
        let print = Print::new(true);
        let err = materialize_source(&meta, None, dir.path(), &print)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::SourceUriRequired));
    }
}
