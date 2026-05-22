use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use regex::Regex;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract::Spec;
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

use crate::{
    commands::{
        container,
        contract::build::verifiable::{
            self, bldimg_regex, source_sha256_regex, source_uri_regex,
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

    /// Override the default docker host used by the rebuild.
    #[arg(short = 'd', long, env = "DOCKER_HOST")]
    pub docker_host: Option<String>,

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

    #[error(transparent)]
    Verifiable(#[from] verifiable::Error),

    #[error(transparent)]
    Bollard(#[from] bollard::errors::Error),

    #[error(transparent)]
    DockerConnection(#[from] container::shared::Error),

    #[error("could not find a rebuilt WASM under {target}")]
    NoRebuiltWasm { target: PathBuf },

    #[error("multiple rebuilt WASMs under {target}; pass --package=... in the bldopt entries to disambiguate. Found: {found}")]
    AmbiguousRebuiltWasm { target: PathBuf, found: String },

    #[error("reading rebuilt wasm {path}: {source}")]
    ReadRebuilt {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("verification failed: rebuilt bytes do not match the original.\n  original: {original_size} bytes, sha256={original_hash}\n  rebuilt:  {rebuilt_size} bytes, sha256={rebuilt_hash}")]
    VerificationMismatch {
        original_hash: String,
        original_size: usize,
        rebuilt_hash: String,
        rebuilt_size: usize,
    },
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
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        let wasm_bytes = self.fetch_wasm().await?;
        let meta = extract_metadata(&wasm_bytes)?;

        print.infoln(format!("Build image: {}", meta.bldimg));
        if let Some(v) = &meta.source_uri {
            print.infoln(format!("Source URI: {v}"));
        }
        if let Some(v) = &meta.source_sha256 {
            print.infoln(format!("Source SHA-256: {v}"));
        }

        if !meta.bldopts.is_empty() {
            print.infoln(format!("Build options ({}):", meta.bldopts.len()));
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

        // Materialize the recorded source into a tempdir so the rebuild can
        // bind-mount it. TempDir lives across the rebuild + comparison and
        // cleans up on drop.
        let workdir = tempfile::TempDir::new().map_err(Error::TempDir)?;
        materialize_source(&meta, self.source_uri.as_deref(), workdir.path(), &print).await?;
        print.checkln(format!(
            "Source materialized at {}",
            workdir.path().display()
        ));

        // Rebuild in the recorded bldimg.
        let docker_args = container::shared::Args {
            docker_host: self.docker_host.clone(),
        };
        let docker = docker_args.connect_to_docker(&print).await?;
        verifiable::pull_image(&docker, &meta.bldimg, &print).await?;
        let container_cmd = build_container_command(&meta);
        verifiable::run_in_container(
            &meta.bldimg,
            workdir.path(),
            &[container_cmd],
            &[],
            &docker,
            &print,
            global_args.verbose || global_args.very_verbose,
        )
        .await?;

        // Locate the rebuilt WASM. The cargo target dir lives under the bind-
        // mounted /source, which we mapped to `workdir`.
        let rebuilt_path = find_rebuilt_wasm(workdir.path(), &meta)?;
        let rebuilt = std::fs::read(&rebuilt_path).map_err(|e| Error::ReadRebuilt {
            path: rebuilt_path.clone(),
            source: e,
        })?;

        // Compare. The final result is always shown, even under `--quiet`,
        // via a dedicated Print that ignores the quiet flag.
        let result_print = Print::new(false);
        let original_hash = format!("{:x}", Sha256::digest(&wasm_bytes));
        let rebuilt_hash = format!("{:x}", Sha256::digest(&rebuilt));
        if original_hash == rebuilt_hash && wasm_bytes.len() == rebuilt.len() {
            result_print.checkln(format!(
                "Verified: {} bytes, sha256={original_hash}",
                wasm_bytes.len()
            ));
            Ok(())
        } else {
            Err(Error::VerificationMismatch {
                original_hash,
                original_size: wasm_bytes.len(),
                rebuilt_hash,
                rebuilt_size: rebuilt.len(),
            })
        }
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
                "Trusting {kind} {value} because --trust was passed"
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
    // Trust prompts must be visible even under `--quiet` so the user can see
    // what they're agreeing to. Use a dedicated Print that ignores the flag.
    let print = Print::new(false);
    let context = match kind {
        TrustKind::Bldimg => format!(
            "Image {value} is not in the default trust list (only docker.io/stellar/stellar-cli is trusted by default)."
        ),
        TrustKind::Tarball => format!(
            "Tarball source {value} is not trusted by default. Tarballs always require confirmation."
        ),
    };
    print.warnln(context);
    print.question(format!("Trust this {kind} and continue? [y/N] "));
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
        print.checkln("Source SHA-256 matches");
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

/// Compose the argv we hand to the container's `stellar contract build` so
/// that:
///   - the bldopts from the original build become flags (each entry is one
///     token, ready for clap), AND
///   - bldimg / source-ids / bldopt are re-recorded as `--meta` entries so
///     the rebuilt WASM has identical metadata to the original.
///
/// cliver is intentionally not re-injected — the container's stellar adds it
/// automatically, and it will match the original's iff `bldimg` resolves to
/// the same container.
fn build_container_command(meta: &ExtractedMetadata) -> Vec<String> {
    let mut forwarded: Vec<String> = meta.bldopts.clone();
    let mut metadata: Vec<String> = Vec::new();

    let mut push_meta = |k: &str, v: &str| {
        metadata.push("--meta".to_string());
        metadata.push(format!("{k}={v}"));
    };
    push_meta("bldimg", &meta.bldimg);
    if let Some(v) = &meta.source_uri {
        push_meta("source_uri", v);
    }
    if let Some(v) = &meta.source_sha256 {
        push_meta("source_sha256", v);
    }
    for o in &meta.bldopts {
        push_meta("bldopt", o);
    }

    // `--locked` is always sent — even if the original somehow lacked it (a
    // non-conformant build), the verifier insists on a locked rebuild so
    // dependency drift can't move bytes underneath us.
    if !forwarded.iter().any(|a| a == "--locked") {
        forwarded.insert(0, "--locked".to_string());
    }

    verifiable::compose_container_args(&forwarded, &metadata)
}

/// Locate the rebuilt WASM under `workdir`. The container writes to
/// `<workdir>/target/wasm32v1-none/release/<pkg>.wasm` (or `wasm32-unknown-unknown/release`
/// for older toolchains; check both). If a `--package=<name>` bldopt was
/// recorded, prefer that file.
fn find_rebuilt_wasm(workdir: &Path, meta: &ExtractedMetadata) -> Result<PathBuf, Error> {
    let preferred_pkg = meta
        .bldopts
        .iter()
        .find_map(|opt| opt.strip_prefix("--package=").map(|s| s.replace('-', "_")));

    let candidates = [
        workdir.join("target/wasm32v1-none/release"),
        workdir.join("target/wasm32-unknown-unknown/release"),
    ];

    let mut found: Vec<PathBuf> = Vec::new();
    for dir in &candidates {
        if !dir.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(dir).map_err(|e| Error::ReadRebuilt {
            path: dir.clone(),
            source: e,
        })? {
            let p = entry
                .map_err(|e| Error::ReadRebuilt {
                    path: dir.clone(),
                    source: e,
                })?
                .path();
            if p.extension().and_then(|s| s.to_str()) == Some("wasm") {
                found.push(p);
            }
        }
    }

    if let Some(pkg) = &preferred_pkg {
        let want = format!("{pkg}.wasm");
        if let Some(p) = found.iter().find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n == want)
        }) {
            return Ok(p.clone());
        }
    }

    match found.len() {
        0 => Err(Error::NoRebuiltWasm {
            target: workdir.join("target"),
        }),
        1 => Ok(found.into_iter().next().unwrap()),
        _ => Err(Error::AmbiguousRebuiltWasm {
            target: workdir.join("target"),
            found: found
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
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

    #[test]
    fn expand_source_repo_rewrites_github_shorthand() {
        assert_eq!(
            expand_source_repo("github:foo/bar"),
            "https://github.com/foo/bar"
        );
    }

    #[test]
    fn expand_source_repo_passes_through_https() {
        assert_eq!(
            expand_source_repo("https://github.com/foo/bar"),
            "https://github.com/foo/bar"
        );
        assert_eq!(
            expand_source_repo("https://gitlab.com/foo/bar.git"),
            "https://gitlab.com/foo/bar.git"
        );
    }

    #[test]
    fn expand_source_repo_does_not_expand_malformed_github() {
        // Missing the `/repo` suffix; the regex won't match so we pass through.
        assert_eq!(expand_source_repo("github:foo"), "github:foo");
        // Extra path component; same.
        assert_eq!(
            expand_source_repo("github:foo/bar/baz"),
            "github:foo/bar/baz"
        );
    }

    #[test]
    fn build_container_command_replays_bldopts_and_re_records_meta() {
        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts: vec![
                "--locked".to_string(),
                "--meta=home_domain=fnando.com".to_string(),
                "--optimize".to_string(),
            ],
        };
        let cmd = build_container_command(&meta);

        // Subcommand prefix.
        assert_eq!(&cmd[..2], &["contract".to_string(), "build".to_string()]);

        // Bldopts are forwarded verbatim as flags to the inner `stellar contract build`.
        assert!(cmd.contains(&"--locked".to_string()));
        assert!(cmd.contains(&"--meta=home_domain=fnando.com".to_string()));
        assert!(cmd.contains(&"--optimize".to_string()));

        // bldimg and source-ids are re-recorded as `--meta`.
        assert!(cmd
            .windows(2)
            .any(|w| w[0] == "--meta" && w[1] == format!("bldimg={}", good_bldimg())));
        assert!(cmd
            .windows(2)
            .any(|w| w[0] == "--meta" && w[1] == "source_uri=https://github.com/foo/bar"));

        // Every bldopt is also re-recorded as a `bldopt=` meta so the rebuilt
        // WASM mirrors the original's entries.
        assert!(cmd
            .windows(2)
            .any(|w| w[0] == "--meta" && w[1] == "bldopt=--locked"));
    }

    #[test]
    fn build_container_command_injects_locked_when_missing() {
        // A non-conformant origin might not have --locked in bldopts. Verify
        // forces it anyway so dependency drift cannot move bytes.
        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts: vec!["--meta=author=alice".to_string()],
        };
        let cmd = build_container_command(&meta);
        let locked_count = cmd.iter().filter(|s| *s == "--locked").count();
        assert_eq!(
            locked_count, 1,
            "expected exactly one --locked, got {locked_count} in {cmd:?}"
        );
    }

    #[test]
    fn find_rebuilt_wasm_picks_single() {
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        std::fs::create_dir_all(&release).unwrap();
        std::fs::write(release.join("hello.wasm"), b"x").unwrap();

        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts: vec![],
        };
        let p = find_rebuilt_wasm(dir.path(), &meta).unwrap();
        assert!(p.ends_with("hello.wasm"));
    }

    #[test]
    fn find_rebuilt_wasm_disambiguates_by_package() {
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        std::fs::create_dir_all(&release).unwrap();
        std::fs::write(release.join("hello.wasm"), b"x").unwrap();
        std::fs::write(release.join("other_thing.wasm"), b"x").unwrap();

        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts: vec!["--package=other-thing".to_string()],
        };
        let p = find_rebuilt_wasm(dir.path(), &meta).unwrap();
        assert!(p.ends_with("other_thing.wasm"));
    }

    #[test]
    fn find_rebuilt_wasm_errors_when_ambiguous_without_package() {
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        std::fs::create_dir_all(&release).unwrap();
        std::fs::write(release.join("hello.wasm"), b"x").unwrap();
        std::fs::write(release.join("other.wasm"), b"x").unwrap();

        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts: vec![],
        };
        let err = find_rebuilt_wasm(dir.path(), &meta).unwrap_err();
        assert!(matches!(err, Error::AmbiguousRebuiltWasm { .. }));
    }

    #[test]
    fn find_rebuilt_wasm_errors_when_none() {
        let dir = tempfile::TempDir::new().unwrap();
        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts: vec![],
        };
        let err = find_rebuilt_wasm(dir.path(), &meta).unwrap_err();
        assert!(matches!(err, Error::NoRebuiltWasm { .. }));
    }
}
