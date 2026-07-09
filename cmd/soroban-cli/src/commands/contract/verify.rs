use std::collections::HashSet;
use std::io::{Cursor, IsTerminal, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use regex::Regex;
use sha2::{Digest, Sha256};
use stellar_xdr::{Hash, Limited, Limits, ReadXdr, ScMetaEntry, ScMetaV0};
use url::Url;
use walkdir::WalkDir;

use crate::{
    commands::{
        container,
        contract::build::{
            source_archive,
            verifiable::{self, bldimg_regex, source_sha256_regex, source_uri_regex},
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
    #[arg(
        long = "id",
        env = "STELLAR_CONTRACT_ID",
        conflicts_with_all = ["wasm", "wasm_hash"]
    )]
    pub contract_id: Option<config::UnresolvedContract>,

    /// Local WASM file to verify, instead of fetching from the network.
    #[arg(long, conflicts_with = "wasm_hash")]
    pub wasm: Option<PathBuf>,

    /// WASM hash (hex) to fetch the WASM from the network.
    #[arg(long = "wasm-hash")]
    pub wasm_hash: Option<String>,

    /// Local source code file or http(s) URL to use as the source when the WASM's
    /// recorded SEP-58 metadata has only `source_sha256` (no `source_uri`).
    /// Accepts http(s) URLs or local file paths.
    #[arg(long)]
    pub source_uri: Option<String>,

    /// Bypass interactive confirmation when the WASM's bldimg is not in the
    /// default trust list, or when the source is provided as an archive (source
    /// archives are never default-trusted).
    #[arg(long)]
    pub trust: bool,

    /// Override the default docker host used by the rebuild.
    #[arg(short = 'd', long, env = "DOCKER_HOST")]
    pub docker_host: Option<String>,

    /// Keep the materialized source and rebuild output instead of deleting them
    /// on exit, and print the path. Useful for debugging a byte mismatch (e.g.
    /// diffing the rebuilt WASM's metadata against the original).
    #[arg(long)]
    pub keep: bool,

    #[command(flatten)]
    pub locator: locator::Args,

    #[command(flatten)]
    pub network: network::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("must pass exactly one of --id, --wasm, or --wasm-hash")]
    MissingInput,

    #[error("invalid wasm hash {0:?}: expected 64 hex characters")]
    InvalidWasmHash(String),

    #[error("reading wasm {0}: {1}")]
    ReadWasm(PathBuf, std::io::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Wasm(#[from] wasm::Error),

    #[error("parsing the WASM's contract metadata: {0}")]
    MetaParse(String),

    #[error("the WASM has no contractmetav0 custom section")]
    NoMeta,

    #[error("the WASM's contractmetav0 does not record a `bldimg` entry; cannot verify")]
    MissingBldimg,

    #[error("the WASM's contractmetav0 records more than one `{field}` entry; refusing to verify (which value applies is ambiguous)")]
    DuplicateMeta { field: &'static str },

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

    #[error("source {uri:?} has an unsupported format; accepted formats are {formats}")]
    UnsupportedSourceFormat { uri: String, formats: String },

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

    #[error("reading extracted source at {path}: {source}")]
    SourceExtract {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("source archive at {path} does not contain exactly one top-level directory (found {count}); SEP-58 requires the source be wrapped in a single directory")]
    SourceArchiveLayout { path: PathBuf, count: usize },

    #[error(transparent)]
    SourceArchive(#[from] source_archive::Error),

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
    SourceArchive,
}

impl std::fmt::Display for TrustKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustKind::Bldimg => write!(f, "bldimg"),
            TrustKind::SourceArchive => write!(f, "source archive"),
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

/// Pure trust decision; no I/O. Source archives are never default-trusted.
pub fn trust_decision(value: &str, kind: TrustKind, trust_flag: bool) -> TrustDecision {
    let default_trusted = match kind {
        TrustKind::Bldimg => trusted_bldimg_regex().is_match(value),
        TrustKind::SourceArchive => false,
    };
    if default_trusted {
        TrustDecision::Trusted
    } else if trust_flag {
        TrustDecision::Overridden
    } else {
        TrustDecision::NeedsConfirmation
    }
}

/// Meta keys the rebuild regenerates on its own, so verify must never replay
/// them — re-passing one would write it twice and break byte-equality. `cliver`
/// is re-injected by the container's CLI; `rsver`/`rssdkver` are re-embedded by
/// the SDK on recompile. The section split in `extract_metadata` already keeps
/// the SDK's own section out; this filter is applied to the chosen section as a
/// final guard (chiefly for a degenerate single-section WASM). Source-embedded
/// keys with arbitrary names (e.g. a `contractmeta!` `Description`) are handled
/// by the section split, which no fixed list could enumerate.
const REGENERATED_META_KEYS: &[&str] = &["cliver", "rsver", "rssdkver"];

/// The `cliver` entry the CLI stamps into the section it injects; its presence
/// marks that section as the CLI-injected one (see `extract_metadata`).
const CLIVER_KEY: &str = "cliver";

/// Metadata read from a contract's `contractmetav0` custom sections (SEP-46).
///
/// Verify reproduces the section by *replaying* what the WASM records rather
/// than reconstructing it from `build`'s ordering rules: `meta_entries` holds
/// the CLI-injected entries, in the exact order the WASM records them, so the
/// rebuild's metadata matches byte-for-byte no matter how (or by what tool) the
/// original was produced. The entries the rebuild regenerates itself — the
/// SDK/compile-emitted section (`rsver`, `rssdkver`, and any `contractmeta!`
/// keys such as `Description`) and the CLI's own `cliver` — are excluded, so
/// they aren't written twice.
///
/// The typed fields (`bldimg`, `source_uri`, `source_sha256`, `bldopts`) are
/// pulled out of the same entries only to *drive* the rebuild — pick the image,
/// trust-check, fetch the source, and derive the forwarded build flags. They are
/// not re-added to the `--meta` list; the replay of `meta_entries` covers them.
#[derive(Debug, Clone)]
pub struct ExtractedMetadata {
    pub bldimg: String,
    pub source_uri: Option<String>,
    pub source_sha256: Option<String>,
    pub bldopts: Vec<String>,
    pub meta_entries: Vec<(String, String)>,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        let wasm_bytes = self.fetch_wasm().await?;
        let meta = extract_metadata(&wasm_bytes)?;

        print.infoln(format!("Build image: {}", meta.bldimg));
        // Report the source we'll actually fetch from. When `--source-uri`
        // overrides the recorded value, show the override (and the recorded
        // value it replaces) so the line isn't misleading.
        match (&self.source_uri, &meta.source_uri) {
            (Some(override_uri), Some(recorded)) => {
                print.infoln(format!(
                    "Source URI: {override_uri} (overrides recorded {recorded})"
                ));
            }
            (Some(override_uri), None) => {
                print.infoln(format!("Source URI: {override_uri} (override)"));
            }
            (None, Some(recorded)) => {
                print.infoln(format!("Source URI: {recorded}"));
            }
            (None, None) => {}
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

        // Catch the no-retrieval-channel case before any trust prompts so a
        // doomed run errors immediately instead of asking the user to trust
        // an image we won't end up using. With only `source_sha256` recorded
        // and no `--source-uri` override, there's nowhere to fetch from.
        if self.effective_source_uri(&meta).is_none() {
            return Err(Error::SourceUriRequired);
        }

        // bldimg trust check is always required.
        require_trust(self.trust, TrustKind::Bldimg, &meta.bldimg, &print)?;

        // Source archive: trust the URL we will actually fetch from (either the
        // value the WASM recorded, or the user's `--source-uri` override).
        if let Some(url) = self.effective_source_uri(&meta) {
            require_trust(self.trust, TrustKind::SourceArchive, &url, &print)?;
        }

        // Materialize the recorded source into a tempdir so the rebuild can
        // bind-mount it. Normally the TempDir cleans up on drop; with `--keep`
        // we persist it (below) so a mismatch can be inspected afterwards.
        let workdir = materialize_source(&meta, self.source_uri.as_deref(), &print).await?;
        print.checkln(format!(
            "Source materialized at {}",
            workdir.path().display()
        ));

        let result = self
            .rebuild_and_verify(workdir.path(), &meta, &wasm_bytes, global_args, &print)
            .await;

        // Persist the build tree when asked — regardless of the outcome, so a
        // byte mismatch (or a rebuild error) can be debugged against the kept
        // source and rebuilt WASM. Otherwise it cleans up on drop here.
        if self.keep {
            let kept = workdir.keep();
            Print::new(false).infoln(format!("Kept build directory at {}", kept.display()));
        }

        result
    }

    /// Rebuild the contract in the recorded `bldimg` and compare the freshly
    /// built WASM against the original. Split out from `run` so the caller owns
    /// the `TempDir` and can keep or drop it after this returns (see `--keep`).
    async fn rebuild_and_verify(
        &self,
        workdir: &Path,
        meta: &ExtractedMetadata,
        wasm_bytes: &[u8],
        global_args: &global::Args,
        print: &Print,
    ) -> Result<(), Error> {
        // Rebuild in the recorded bldimg.
        let docker_args = container::shared::Args {
            docker_host: self.docker_host.clone(),
        };
        let docker = docker_args.connect_to_docker(print).await?;
        verifiable::pull_image(&docker, &meta.bldimg, print).await?;

        // `--locked` was only added to `contract build` in cli 25.2.0. The
        // recorded bldimg may be older (and still valid), so probe it before
        // forcing `--locked` — passing an unknown flag would fail the rebuild.
        let supports_locked = verifiable::probe_supports_locked(&meta.bldimg, &docker, print).await;
        let (container_cmd, env) = build_container_command(meta, supports_locked);

        // SEP-58 requires the source be wrapped in a single top-level directory
        // (the cli names it `source/`, but the spec doesn't fix the name), so
        // the build's working tree is that wrapper dir under `workdir`.
        let source_root = locate_extracted_source_root(workdir)?;

        // Snapshot any WASM artifacts already present in the materialized source
        // *before* the rebuild. A conformant source archive ships no build
        // output, so anything here was planted; excluding these from the post-
        // build search stops an attacker from smuggling a pre-built binary into
        // the archive to masquerade as the rebuild's output and spoof a match.
        let preexisting_wasms: HashSet<PathBuf> =
            collect_release_wasms(&source_root).into_iter().collect();
        if !preexisting_wasms.is_empty() {
            print.warnln(format!(
                "Ignoring {} pre-existing WASM artifact(s) in the source; only freshly rebuilt output is trusted",
                preexisting_wasms.len()
            ));
        }

        verifiable::run_in_container(
            &meta.bldimg,
            &source_root,
            &[container_cmd],
            &env,
            &docker,
            print,
            global_args.verbose || global_args.very_verbose,
        )
        .await?;

        // Locate the rebuilt WASM. The cargo target dir lives under the bind-
        // mounted /source, which we mapped to `source_root`.
        let rebuilt_path = find_rebuilt_wasm(&source_root, meta, &preexisting_wasms)?;
        let rebuilt = std::fs::read(&rebuilt_path).map_err(|e| Error::ReadRebuilt {
            path: rebuilt_path.clone(),
            source: e,
        })?;
        if self.keep {
            print.infoln(format!("Rebuilt WASM at {}", rebuilt_path.display()));
        }

        // Compare. The final result is always shown, even under `--quiet`,
        // via a dedicated Print that ignores the quiet flag.
        let result_print = Print::new(false);
        let original_hash = format!("{:x}", Sha256::digest(wasm_bytes));
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

    /// The source archive URL we'll actually retrieve from: the cli override if set,
    /// otherwise the value recorded in the WASM. Returns `None` when neither
    /// records a `source_uri` (only `source_sha256` is set), in which case
    /// there's nothing to trust-check here.
    fn effective_source_uri(&self, meta: &ExtractedMetadata) -> Option<String> {
        self.source_uri.clone().or_else(|| meta.source_uri.clone())
    }

    async fn fetch_wasm(&self) -> Result<Vec<u8>, Error> {
        // Clap keeps these three mutually exclusive, so at most one is set.
        if let Some(path) = &self.wasm {
            return std::fs::read(path).map_err(|e| Error::ReadWasm(path.clone(), e));
        }
        if let Some(id) = &self.contract_id {
            let network = self.network.get(&self.locator)?;
            let resolved = id.resolve_contract_id(&self.locator, &network.network_passphrase)?;
            return Ok(wasm::fetch_from_contract(&resolved, &network).await?);
        }
        if let Some(wasm_hash) = &self.wasm_hash {
            let network = self.network.get(&self.locator)?;
            let bytes: [u8; 32] = hex::decode(wasm_hash)
                .ok()
                .and_then(|b| b.try_into().ok())
                .ok_or_else(|| Error::InvalidWasmHash(wasm_hash.clone()))?;
            return Ok(wasm::fetch_from_wasm_hash(Hash(bytes), &network).await?);
        }
        Err(Error::MissingInput)
    }
}

/// Read the WASM's `contractmetav0` custom sections *separately*, preserving
/// both the per-section grouping and the entry order within each. `Spec::meta`
/// concatenates every section into one flat list; keeping them apart is what
/// lets verify tell the SDK/compile-emitted metadata (its own section) from the
/// CLI-injected metadata (a separate section appended by `inject_meta`), so it
/// replays only the latter. SEP-46 permits multiple same-named sections and
/// fixes their concatenation order, so this grouping is well-defined.
fn read_meta_sections(wasm: &[u8]) -> Result<Vec<Vec<(String, String)>>, Error> {
    let mut sections = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        let payload = payload.map_err(|e| Error::MetaParse(e.to_string()))?;
        if let wasmparser::Payload::CustomSection(reader) = payload {
            if reader.name() == "contractmetav0" {
                sections.push(parse_meta_entries(reader.data())?);
            }
        }
    }
    Ok(sections)
}

/// Decode one `contractmetav0` section's XDR into `(key, value)` pairs, in order.
fn parse_meta_entries(data: &[u8]) -> Result<Vec<(String, String)>, Error> {
    let mut read = Limited::new(Cursor::new(data), Limits::none());
    ScMetaEntry::read_xdr_iter(&mut read)
        .map(|entry| {
            entry.map(|ScMetaEntry::ScMetaV0(ScMetaV0 { key, val })| {
                (key.to_string(), val.to_string())
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::MetaParse(e.to_string()))
}

/// Read the WASM's contract metadata and split out what verify must replay from
/// what the rebuild regenerates on its own.
///
/// The rebuild re-creates the SDK/compile-emitted metadata (`rsver`, `rssdkver`,
/// and any `contractmeta!` keys) by recompiling the source, and the container's
/// CLI re-injects `cliver`. Replaying any of those as `--meta` would write them
/// twice and break byte-equality. `inject_meta` puts `cliver` plus the user's
/// `--meta` into its own `contractmetav0` section, so the section containing
/// `cliver` *is* the CLI-injected set — everything verify must replay, and
/// nothing the rebuild produces for free. We therefore replay that section
/// (minus `cliver`) and ignore the rest.
///
/// Fallback: a WASM with no `cliver` (a pre-v23.2.0 CLI never wrote one, and a
/// WASM may be hand-authored per SEP-46) has no marked section, so we take the
/// last non-empty section instead — `inject_meta` always appends after the
/// compile-emitted sections, so the CLI section is always last.
///
/// Errors when `bldimg` or `source_sha256` is absent, since neither has a
/// sensible default; `source_uri` is optional.
pub fn extract_metadata(wasm: &[u8]) -> Result<ExtractedMetadata, Error> {
    let sections = read_meta_sections(wasm)?;
    if sections.iter().all(Vec::is_empty) {
        return Err(Error::NoMeta);
    }

    // Locate the CLI-injected section: the one carrying `cliver`, or — when no
    // section is marked — the last non-empty one, since `inject_meta` always
    // appends after the compile-emitted sections (the linker merges every
    // `#[link_section = "contractmetav0"]` static — `contractmeta!` entries plus
    // the SDK's `rsver`/`rssdkver` — into a single earlier section). Replay it,
    // dropping the keys the rebuild regenerates itself (`REGENERATED_META_KEYS`):
    // a well-formed CLI section holds none of them, but this guards a degenerate
    // single-section WASM where build fields sit alongside `rsver`/`rssdkver`.
    let cli_section = sections
        .iter()
        .position(|s| s.iter().any(|(k, _)| k == CLIVER_KEY))
        .or_else(|| sections.iter().rposition(|s| !s.is_empty()))
        .expect("a non-empty section exists: the all-empty case is rejected as NoMeta above");
    let meta_entries: Vec<(String, String)> = sections[cli_section]
        .iter()
        .filter(|(k, _)| !REGENERATED_META_KEYS.contains(&k.as_str()))
        .cloned()
        .collect();

    let mut bldimg: Option<String> = None;
    let mut source_uri: Option<String> = None;
    let mut source_sha256: Option<String> = None;
    let mut bldopts: Vec<String> = Vec::new();

    // Each of these fields must appear at most once. Reject duplicates rather
    // than silently taking the last: two `bldimg` entries (say a benign one to
    // fool inspection and a second the cli would actually trust and rebuild in)
    // would be a verification-bypass vector, and the same ambiguity applies to
    // the `source_uri`/`source_sha256` that pin what gets rebuilt.
    let set_once =
        |slot: &mut Option<String>, field: &'static str, v: String| -> Result<(), Error> {
            if slot.is_some() {
                return Err(Error::DuplicateMeta { field });
            }
            *slot = Some(v);
            Ok(())
        };

    // The typed fields are pulled out of the very entries we replay, so the
    // rebuild is driven by exactly the metadata that gets re-recorded.
    for (k, v) in &meta_entries {
        match k.as_str() {
            "bldimg" => set_once(&mut bldimg, "bldimg", v.clone())?,
            "source_uri" => set_once(&mut source_uri, "source_uri", v.clone())?,
            "source_sha256" => set_once(&mut source_sha256, "source_sha256", v.clone())?,
            "bldopt" => bldopts.push(v.clone()),
            _ => {} // user meta: carried in meta_entries for replay
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
        meta_entries,
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
        TrustKind::SourceArchive => format!(
            "Source archive {value} is not trusted by default. Source archives always require confirmation."
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

/// Materialize the recorded source tree into a fresh, permission-hardened
/// tempdir and return the guard. The retrieval channel is the cli's
/// `--source-uri` flag (when set) or the WASM's recorded `source_uri`; either
/// may be an http(s) URL or a local file path. When the bytes are present, the
/// optional `source_sha256` is checked before extraction.
///
/// Extraction (under the data dir, hardened) is shared with `build
/// --verifiable` via `source_archive::extract_into_hardened_tempdir`.
async fn materialize_source(
    meta: &ExtractedMetadata,
    source_uri_override: Option<&str>,
    print: &Print,
) -> Result<tempfile::TempDir, Error> {
    let resolved_source = source_uri_override
        .map(str::to_string)
        .or_else(|| meta.source_uri.clone());
    let Some(source) = resolved_source else {
        // No source_uri anywhere — only source_sha256 is set.
        return Err(Error::SourceUriRequired);
    };

    let format = resolve_source_format(&source)?;

    print.infoln(format!("Fetching source code from {source}"));
    let bytes = fetch_source_bytes(&source).await?;
    if let Some(expected) = &meta.source_sha256 {
        verify_source_sha256(&bytes, expected)?;
        print.checkln("Source SHA-256 matches");
    }
    Ok(source_archive::extract_into_hardened_tempdir(
        &bytes,
        "verify-src-",
        format,
    )?)
}

/// The last path segment of `source`, whether it's a URL or a local path. Try
/// parsing as a URL first (so query strings and fragments are dropped); if that
/// fails, `source` is a local path, so fall back to `Path::file_name`.
fn source_basename(source: &str) -> String {
    if let Ok(url) = Url::parse(source) {
        return url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .unwrap_or_default()
            .to_string();
    }
    Path::new(source)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Determine the archive format from a `--source-uri` (or recorded `source_uri`)
/// by its basename, rejecting sources whose extension we don't recognize before
/// we bother fetching them, naming the formats we accept.
fn resolve_source_format(source: &str) -> Result<source_archive::ArchiveFormat, Error> {
    let basename = source_basename(source);
    source_archive::ArchiveFormat::from_filename(&basename).ok_or_else(|| {
        Error::UnsupportedSourceFormat {
            uri: source.to_string(),
            formats: source_archive::ArchiveFormat::recognized_extensions(),
        }
    })
}

/// Retrieve the source archive bytes. `source` is either an `http(s)://` URL or
/// a local file path. The split is by prefix, not by attempting both — keeps
/// behavior predictable.
async fn fetch_source_bytes(source: &str) -> Result<Vec<u8>, Error> {
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

/// SEP-58 requires the source archive wrap everything in a single top-level
/// directory (the cli names it `source/`, but the spec leaves the name open),
/// so after extraction the build tree is that lone directory under `workdir`.
/// Return it, erroring if the archive doesn't have exactly one top-level dir.
fn locate_extracted_source_root(workdir: &Path) -> Result<PathBuf, Error> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(workdir)
        .map_err(|source| Error::SourceExtract {
            path: workdir.to_path_buf(),
            source,
        })?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();

    match dirs.len() {
        1 => Ok(dirs.remove(0)),
        count => Err(Error::SourceArchiveLayout {
            path: workdir.to_path_buf(),
            count,
        }),
    }
}

/// Compose the argv we hand to the container's `stellar contract build`, plus
/// the env vars to apply via docker `-e`.
///
/// The metadata is *replayed*, not reconstructed: every entry the WASM records
/// (`meta.meta_entries`, already stripped of the keys the rebuild regenerates)
/// is re-emitted as a `--meta key=value` in its original order, so the rebuilt
/// `contractmetav0` mirrors the source WASM regardless of how it was produced.
/// This keeps verify independent of `build`'s ordering rules and lets it verify
/// WASMs authored by hand per SEP-58.
///
/// The `bldopt` entries additionally drive the *build flags*: each is forwarded
/// as a flag to the inner `contract build`, with two exceptions —
///   - `--env=` bldopts are applied via docker `-e` (as the original build did),
///     not forwarded. They're shell-escaped at the source, so we unescape back
///     to a raw `NAME=VALUE`.
///   - `--meta=` bldopts are NOT forwarded: the metadata they produced is
///     already a standalone entry in `meta_entries` and replayed above, so
///     forwarding them too would write the value twice.
///
/// Both are still re-recorded — via their `bldopt=` entry in `meta_entries`.
///
/// `supports_locked`: whether the recorded bldimg's `contract build` accepts
/// `--locked` (added in cli 25.2.0). When false the flag is never injected, so
/// a rebuild against an older image doesn't fail on an unknown argument.
fn build_container_command(
    meta: &ExtractedMetadata,
    supports_locked: bool,
) -> (Vec<String>, Vec<String>) {
    let mut forwarded: Vec<String> = Vec::new();
    let mut env: Vec<String> = Vec::new();
    for o in &meta.bldopts {
        // Every recorded bldopt is shell-escaped at the source (see
        // `build_forwarded_args` in verifiable.rs) so it's valid shell on its
        // own — e.g. `--meta=source_repo='github:foo'` or `--env=B='a b'`. The
        // single-package rebuild hands argv straight to `stellar` with no shell,
        // so unescape each bldopt back to the one raw argv token the original
        // build used; otherwise the quoting leaks into the value.
        let token = shlex::split(o)
            .and_then(|mut v| (v.len() == 1).then(|| v.remove(0)))
            .unwrap_or_else(|| o.clone());
        if let Some(kv) = token.strip_prefix("--env=") {
            // Applied via docker `-e` as a raw `NAME=VALUE`, never forwarded.
            env.push(kv.to_string());
        } else if token.starts_with("--meta=") {
            // The metadata this produced is replayed from `meta_entries`;
            // forwarding it as a flag too would write the value twice.
        } else {
            forwarded.push(token);
        }
    }

    // When the image supports it, `--locked` is forced — even if the original
    // somehow lacked it (a non-conformant build) — so the verifier insists on a
    // locked rebuild and dependency drift can't move bytes underneath us. Older
    // images (< cli 25.2.0) reject the flag, so it's omitted there. Forcing it
    // only affects the build (determinism); it isn't recorded as metadata, so it
    // doesn't perturb the rebuilt `contractmetav0`.
    if supports_locked && !forwarded.iter().any(|a| a == "--locked") {
        forwarded.insert(0, "--locked".to_string());
    }

    // Replay every recorded meta entry verbatim, in the WASM's own order, so the
    // rebuilt section matches the original byte-for-byte.
    let mut metadata: Vec<String> = Vec::new();
    for (k, v) in &meta.meta_entries {
        metadata.push("--meta".to_string());
        metadata.push(format!("{k}={v}"));
    }

    let mut args = vec!["contract".to_string(), "build".to_string()];
    args.extend(forwarded);
    args.extend(metadata);
    (args, env)
}

/// The two wasm release-output suffixes cargo may write to, newest first.
/// Older toolchains build for `wasm32-unknown-unknown`; current ones use
/// `wasm32v1-none`. The match is deliberately the 2-component `<triple>/release`
/// tail rather than `target/<triple>/release`: cargo's target dir is not fixed
/// at `target/` (it can be relocated via `--target-dir`, `CARGO_TARGET_DIR`, or
/// `build.target-dir`), but the `<triple>/release/` layout beneath it is
/// stable. Matching the tail also excludes intermediate artifacts under
/// `release/deps/`, whose parent ends with `release/deps`, not `.../release`.
const WASM_RELEASE_SUFFIXES: [&str; 2] =
    ["wasm32v1-none/release", "wasm32-unknown-unknown/release"];

/// Walk `root` and return every `*.wasm` sitting directly in a
/// `<triple>/release` output directory. The target dir's location is not fixed
/// relative to the crate manifest — in a Cargo workspace it lives at the
/// workspace root, which may be any ancestor of the `--manifest-path` crate —
/// so we search the whole tree rather than guess where it is.
fn collect_release_wasms(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("wasm"))
        .filter(|p| {
            p.parent()
                .is_some_and(|parent| WASM_RELEASE_SUFFIXES.iter().any(|s| parent.ends_with(s)))
        })
        .collect()
}

/// Locate the WASM produced by the container's rebuild under `source_root`.
///
/// Only artifacts *created by this rebuild* are eligible: any `*.wasm` present
/// before the build (captured in `preexisting`) is excluded, so a pre-built
/// binary planted in the source archive can't masquerade as the rebuild output.
/// If a `--package=<name>` bldopt was recorded, prefer that file.
fn find_rebuilt_wasm(
    source_root: &Path,
    meta: &ExtractedMetadata,
    preexisting: &HashSet<PathBuf>,
) -> Result<PathBuf, Error> {
    let preferred_pkg = meta
        .bldopts
        .iter()
        .find_map(|opt| opt.strip_prefix("--package=").map(|s| s.replace('-', "_")));

    let found: Vec<PathBuf> = collect_release_wasms(source_root)
        .into_iter()
        .filter(|p| !preexisting.contains(p))
        .collect();

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
            target: source_root.to_path_buf(),
        }),
        1 => Ok(found.into_iter().next().unwrap()),
        _ => Err(Error::AmbiguousRebuiltWasm {
            target: source_root.to_path_buf(),
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
    use stellar_xdr::{Limited, Limits, ScMetaEntry, ScMetaV0, WriteXdr};

    fn make_wasm_with_meta(entries: &[(&str, &str)]) -> Vec<u8> {
        make_wasm_with_sections(&[entries])
    }

    /// Build a WASM with one `contractmetav0` custom section per slice, in order
    /// — mirroring how the SDK/compile step and the CLI's `inject_meta` each
    /// append their own section.
    fn make_wasm_with_sections(sections: &[&[(&str, &str)]]) -> Vec<u8> {
        let mut wasm = empty_wasm_module();
        for entries in sections {
            let xdr = encode_meta(entries);
            wasm_gen::write_custom_section(&mut wasm, "contractmetav0", &xdr);
        }
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
    fn extract_metadata_happy_path_source_pair() {
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
        assert_eq!(meta.source_sha256.as_deref(), Some("f".repeat(64).as_str()));
    }

    #[test]
    fn extract_metadata_missing_bldimg_errors() {
        let wasm = make_wasm_with_meta(&[("source_sha256", &"b".repeat(64))]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::MissingBldimg));
    }

    #[test]
    fn extract_metadata_duplicate_bldimg_errors() {
        // A second bldimg — e.g. a benign one to fool inspection plus one the
        // cli would actually trust and rebuild in — must be rejected outright.
        let other = format!("docker.io/attacker/evil@sha256:{}", "b".repeat(64));
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("bldimg", &other),
            ("source_sha256", &"f".repeat(64)),
        ]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::DuplicateMeta { field: "bldimg" }));
    }

    #[test]
    fn extract_metadata_duplicate_source_ids_error() {
        for field in ["source_uri", "source_sha256"] {
            let wasm = make_wasm_with_meta(&[
                ("bldimg", &good_bldimg()),
                ("source_sha256", &"f".repeat(64)),
                (field, "https://example.com/a.tar.gz"),
                (field, "https://example.com/b.tar.gz"),
            ]);
            let err = extract_metadata(&wasm).unwrap_err();
            assert!(
                matches!(err, Error::DuplicateMeta { field: f } if f == field),
                "expected DuplicateMeta for {field}, got {err:?}"
            );
        }
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
    fn extract_metadata_replays_only_cli_section() {
        // Real layout: the SDK/compile step emits its own section (a
        // `contractmeta!` `Description`, plus rsver/rssdkver), then the CLI
        // appends a second section holding cliver + the user `--meta`. Verify
        // must replay only the CLI section (minus cliver); the source-embedded
        // section is regenerated by recompiling, so replaying it would duplicate
        // those entries and break byte-equality (the bug this fixes).
        let wasm = make_wasm_with_sections(&[
            // SDK / compile-emitted section.
            &[
                ("Description", "A hello world contract"),
                ("key1", "val1"),
                ("key2", "val2"),
                ("rsver", "1.96.0"),
                ("rssdkver", "26.1.0#abcdef"),
            ],
            // CLI-injected section.
            &[
                ("cliver", "27.0.0#abcdef"),
                ("bldimg", &good_bldimg()),
                ("source_sha256", &"b".repeat(64)),
                ("bldopt", "--locked"),
                ("home_domain", "fnando.com"),
            ],
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        assert_eq!(meta.bldopts, vec!["--locked".to_string()]);
        // Only the CLI section is replayed, in order, with cliver stripped.
        // Nothing from the source-embedded section leaks in.
        assert_eq!(
            meta.meta_entries,
            vec![
                ("bldimg".to_string(), good_bldimg()),
                ("source_sha256".to_string(), "b".repeat(64)),
                ("bldopt".to_string(), "--locked".to_string()),
                ("home_domain".to_string(), "fnando.com".to_string()),
            ]
        );
    }

    #[test]
    fn extract_metadata_fallback_picks_last_section_without_cliver() {
        // Pre-v23.2.0 (or hand-authored per SEP-46): no cliver marker anywhere.
        // The build fields were added via plain --meta, so they're in the CLI-
        // appended (last) section; the compile-emitted section — contractmeta!
        // entries plus rsver/rssdkver — comes first and must be ignored, or its
        // key1/key2 would be replayed and duplicated on rebuild.
        let wasm = make_wasm_with_sections(&[
            &[
                ("key1", "val1"),
                ("key2", "val2"),
                ("rsver", "1.97.0"),
                ("rssdkver", "22.0.11#abcdef"),
            ],
            &[
                ("bldimg", &good_bldimg()),
                ("source_sha256", &"b".repeat(64)),
                ("bldopt", "--locked"),
            ],
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        assert_eq!(
            meta.meta_entries,
            vec![
                ("bldimg".to_string(), good_bldimg()),
                ("source_sha256".to_string(), "b".repeat(64)),
                ("bldopt".to_string(), "--locked".to_string()),
            ]
        );
    }

    #[test]
    fn extract_metadata_single_section_fallback_drops_regenerated_keys() {
        // Degenerate: only one section, no cliver, build fields embedded next to
        // rsver/rssdkver. The last-section fallback picks it, and the guard filter
        // still strips rsver/rssdkver so they aren't replayed and duplicated.
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("source_sha256", &"b".repeat(64)),
            ("rsver", "1.97.0"),
            ("rssdkver", "22.0.11#abcdef"),
            ("home_domain", "fnando.com"),
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        assert_eq!(
            meta.meta_entries,
            vec![
                ("bldimg".to_string(), good_bldimg()),
                ("source_sha256".to_string(), "b".repeat(64)),
                ("home_domain".to_string(), "fnando.com".to_string()),
            ]
        );
    }

    #[test]
    fn extract_metadata_ignores_duplicate_key_in_source_embedded_section() {
        // A `contractmeta!` entry that happens to reuse a reserved key (here a
        // second `bldimg`) lives in the source-embedded section, which verify
        // ignores — so it neither trips duplicate-rejection nor overrides the
        // real, CLI-recorded bldimg used to drive (and trust-check) the rebuild.
        let evil = format!("docker.io/attacker/evil@sha256:{}", "e".repeat(64));
        let wasm = make_wasm_with_sections(&[
            &[("bldimg", &evil), ("rsver", "1.96.0")],
            &[
                ("cliver", "27.0.0#abcdef"),
                ("bldimg", &good_bldimg()),
                ("source_sha256", &"b".repeat(64)),
            ],
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        assert_eq!(meta.bldimg, good_bldimg());
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
    fn trust_decision_source_archive_always_needs_confirmation() {
        assert_eq!(
            trust_decision(
                "https://github.com/foo/bar.tar.gz",
                TrustKind::SourceArchive,
                false
            ),
            TrustDecision::NeedsConfirmation
        );
        assert_eq!(
            trust_decision("/local/foo.tar.gz", TrustKind::SourceArchive, false),
            TrustDecision::NeedsConfirmation
        );
    }

    #[test]
    fn trust_decision_source_archive_override_with_trust() {
        assert_eq!(
            trust_decision(
                "https://github.com/foo/bar.tar.gz",
                TrustKind::SourceArchive,
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

    #[tokio::test]
    async fn materialize_source_errors_when_only_source_sha256() {
        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: None,
            source_sha256: Some("f".repeat(64)),
            bldopts: Vec::new(),
            meta_entries: Vec::new(),
        };
        let print = Print::new(true);
        let err = materialize_source(&meta, None, &print).await.unwrap_err();
        assert!(matches!(err, Error::SourceUriRequired));
    }

    #[test]
    fn build_container_command_replays_meta_in_order_and_forwards_build_flags() {
        // The recorded entries as they'd appear in the WASM (cliver/rsver/etc.
        // already excluded by extract_metadata). build_container_command must
        // replay these verbatim, in order, and derive the build flags from the
        // `bldopt=` entries.
        let meta_entries = vec![
            ("bldimg".to_string(), good_bldimg()),
            (
                "source_uri".to_string(),
                "https://github.com/foo/bar".to_string(),
            ),
            ("source_sha256".to_string(), "b".repeat(64)),
            ("home_domain".to_string(), "fnando.com".to_string()),
            ("bldopt".to_string(), "--locked".to_string()),
            (
                "bldopt".to_string(),
                "--meta=home_domain=fnando.com".to_string(),
            ),
            ("bldopt".to_string(), "--optimize".to_string()),
            ("bldopt".to_string(), "--env=A=1".to_string()),
            (
                "bldopt".to_string(),
                "--env=B='this is very nice'".to_string(),
            ),
        ];
        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts: vec![
                "--locked".to_string(),
                "--meta=home_domain=fnando.com".to_string(),
                "--optimize".to_string(),
                "--env=A=1".to_string(),
                "--env=B='this is very nice'".to_string(),
            ],
            meta_entries: meta_entries.clone(),
        };
        let (cmd, env) = build_container_command(&meta, true);

        // Subcommand prefix.
        assert_eq!(&cmd[..2], &["contract".to_string(), "build".to_string()]);

        // Build-affecting bldopts are forwarded as flags to the inner build.
        assert!(cmd.contains(&"--locked".to_string()));
        assert!(cmd.contains(&"--optimize".to_string()));

        // `--meta=` bldopts are NOT forwarded as flags: the value is replayed as
        // its own standalone `home_domain` entry below, so forwarding it too
        // would write it twice.
        assert!(!cmd.iter().any(|a| a.starts_with("--meta=")));

        // `--env=` bldopts are applied via docker `-e` (unescaped), never
        // forwarded as build flags.
        assert!(!cmd.iter().any(|a| a.starts_with("--env=")));
        assert_eq!(
            env,
            vec!["A=1".to_string(), "B=this is very nice".to_string()]
        );

        // The `--meta` list is the recorded entries replayed verbatim, in the
        // exact order the WASM records them.
        let replayed: Vec<(String, String)> = cmd
            .windows(2)
            .filter(|w| w[0] == "--meta")
            .map(|w| {
                let (k, v) = w[1].split_once('=').unwrap();
                (k.to_string(), v.to_string())
            })
            .collect();
        assert_eq!(replayed, meta_entries);
    }

    #[test]
    fn build_container_command_replays_meta_bldopt_verbatim_without_forwarding() {
        // A `--meta=` bldopt is shell-escaped at the source (a value with a `:`
        // or spaces is stored quoted). Verify must NOT forward it as a build
        // flag — the value reaches the rebuilt WASM through the standalone
        // `source_repo` entry — and the `bldopt=` entry itself is replayed
        // verbatim so its escaped form round-trips byte-for-byte.
        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts: vec![
                "--meta=source_repo='github:LayerZero-Labs/monorepo-external'".to_string(),
            ],
            meta_entries: vec![
                (
                    "source_repo".to_string(),
                    "github:LayerZero-Labs/monorepo-external".to_string(),
                ),
                (
                    "bldopt".to_string(),
                    "--meta=source_repo='github:LayerZero-Labs/monorepo-external'".to_string(),
                ),
            ],
        };
        let (cmd, _env) = build_container_command(&meta, true);

        // No `--meta=` bldopt is forwarded as a build flag.
        assert!(
            !cmd.iter().any(|a| a.starts_with("--meta=")),
            "--meta bldopts must not be forwarded, got {cmd:?}"
        );

        // The standalone entry carries the unescaped value to the rebuild.
        assert!(
            cmd.windows(2).any(|w| w[0] == "--meta"
                && w[1] == "source_repo=github:LayerZero-Labs/monorepo-external"),
            "the standalone meta entry must be replayed, got {cmd:?}"
        );

        // The `bldopt=` entry keeps the original escaped form verbatim so the
        // rebuilt WASM's bldopt entry matches the original byte-for-byte.
        assert!(
            cmd.windows(2).any(|w| w[0] == "--meta"
                && w[1] == "bldopt=--meta=source_repo='github:LayerZero-Labs/monorepo-external'"),
            "the bldopt meta must round-trip the escaped original, got {cmd:?}"
        );
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
            meta_entries: vec![
                ("author".to_string(), "alice".to_string()),
                ("bldopt".to_string(), "--meta=author=alice".to_string()),
            ],
        };
        let (cmd, _env) = build_container_command(&meta, true);
        let locked_count = cmd.iter().filter(|s| *s == "--locked").count();
        assert_eq!(
            locked_count, 1,
            "expected exactly one --locked, got {locked_count} in {cmd:?}"
        );
    }

    #[test]
    fn build_container_command_omits_locked_when_unsupported() {
        // Older images (< cli 25.2.0) reject `--locked`; when the probe reports
        // it's unsupported, the flag is never forwarded — even if the original's
        // bldopts recorded it, it still round-trips as a `bldopt` meta only.
        let meta = ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts: vec!["--optimize".to_string()],
            meta_entries: vec![("bldopt".to_string(), "--optimize".to_string())],
        };
        let (cmd, _env) = build_container_command(&meta, false);
        assert!(
            !cmd.iter().any(|a| a == "--locked"),
            "expected no forwarded --locked in {cmd:?}"
        );
    }

    fn meta_with_bldopts(bldopts: Vec<String>) -> ExtractedMetadata {
        ExtractedMetadata {
            bldimg: good_bldimg(),
            source_uri: Some("https://github.com/foo/bar".to_string()),
            source_sha256: Some("b".repeat(64)),
            bldopts,
            meta_entries: Vec::new(),
        }
    }

    #[test]
    fn find_rebuilt_wasm_picks_single() {
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        std::fs::create_dir_all(&release).unwrap();
        std::fs::write(release.join("hello.wasm"), b"x").unwrap();

        let meta = meta_with_bldopts(vec![]);
        let p = find_rebuilt_wasm(dir.path(), &meta, &HashSet::new()).unwrap();
        assert!(p.ends_with("hello.wasm"));
    }

    #[test]
    fn find_rebuilt_wasm_disambiguates_by_package() {
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        std::fs::create_dir_all(&release).unwrap();
        std::fs::write(release.join("hello.wasm"), b"x").unwrap();
        std::fs::write(release.join("other_thing.wasm"), b"x").unwrap();

        let meta = meta_with_bldopts(vec!["--package=other-thing".to_string()]);
        let p = find_rebuilt_wasm(dir.path(), &meta, &HashSet::new()).unwrap();
        assert!(p.ends_with("other_thing.wasm"));
    }

    #[test]
    fn find_rebuilt_wasm_errors_when_ambiguous_without_package() {
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        std::fs::create_dir_all(&release).unwrap();
        std::fs::write(release.join("hello.wasm"), b"x").unwrap();
        std::fs::write(release.join("other.wasm"), b"x").unwrap();

        let meta = meta_with_bldopts(vec![]);
        let err = find_rebuilt_wasm(dir.path(), &meta, &HashSet::new()).unwrap_err();
        assert!(matches!(err, Error::AmbiguousRebuiltWasm { .. }));
    }

    #[test]
    fn find_rebuilt_wasm_errors_when_none() {
        let dir = tempfile::TempDir::new().unwrap();
        let meta = meta_with_bldopts(vec![]);
        let err = find_rebuilt_wasm(dir.path(), &meta, &HashSet::new()).unwrap_err();
        assert!(matches!(err, Error::NoRebuiltWasm { .. }));
    }

    #[test]
    fn find_rebuilt_wasm_finds_target_at_workspace_root() {
        // In a Cargo workspace the `target/` dir sits at the workspace root, not
        // next to the crate manifest. The search must still find it when
        // `--manifest-path` points deep into a subdirectory (the bug that
        // motivated the tree walk).
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        std::fs::create_dir_all(&release).unwrap();
        std::fs::write(release.join("blocked_message_lib.wasm"), b"x").unwrap();
        std::fs::create_dir_all(
            dir.path()
                .join("contracts/message-libs/blocked-message-lib/src"),
        )
        .unwrap();

        let meta = meta_with_bldopts(vec![
            "--manifest-path=contracts/message-libs/blocked-message-lib/Cargo.toml".to_string(),
            "--package=blocked-message-lib".to_string(),
        ]);
        let p = find_rebuilt_wasm(dir.path(), &meta, &HashSet::new()).unwrap();
        assert!(p.ends_with("blocked_message_lib.wasm"));
    }

    #[test]
    fn find_rebuilt_wasm_finds_relocated_target_dir() {
        // The output dir need not be named `target/` (e.g. CARGO_TARGET_DIR).
        // The `<triple>/release/` tail is what's stable, so a renamed dir is
        // still found.
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("custom-out/wasm32-unknown-unknown/release");
        std::fs::create_dir_all(&release).unwrap();
        std::fs::write(release.join("hello.wasm"), b"x").unwrap();

        let meta = meta_with_bldopts(vec![]);
        let p = find_rebuilt_wasm(dir.path(), &meta, &HashSet::new()).unwrap();
        assert!(p.ends_with("hello.wasm"));
    }

    #[test]
    fn find_rebuilt_wasm_ignores_release_deps_artifacts() {
        // Intermediate wasms under `release/deps/` are not the final artifact
        // and must not be matched.
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        let deps = release.join("deps");
        std::fs::create_dir_all(&deps).unwrap();
        std::fs::write(release.join("hello.wasm"), b"x").unwrap();
        std::fs::write(deps.join("hello-abc123.wasm"), b"x").unwrap();

        let meta = meta_with_bldopts(vec![]);
        let p = find_rebuilt_wasm(dir.path(), &meta, &HashSet::new()).unwrap();
        assert!(p.ends_with("hello.wasm"));
    }

    #[test]
    fn find_rebuilt_wasm_excludes_preexisting_injected_wasm() {
        // An attacker ships a pre-built wasm at the output path. It's captured
        // in the pre-build snapshot and excluded, so it can't spoof a match —
        // leaving no eligible rebuild output.
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        std::fs::create_dir_all(&release).unwrap();
        let injected = release.join("hello.wasm");
        std::fs::write(&injected, b"x").unwrap();

        let preexisting: HashSet<PathBuf> = collect_release_wasms(dir.path()).into_iter().collect();
        assert!(preexisting.contains(&injected));

        let meta = meta_with_bldopts(vec![]);
        let err = find_rebuilt_wasm(dir.path(), &meta, &preexisting).unwrap_err();
        assert!(matches!(err, Error::NoRebuiltWasm { .. }));
    }

    #[test]
    fn find_rebuilt_wasm_keeps_freshly_built_alongside_preexisting() {
        // A pre-existing wasm is excluded, but a genuinely new one built next to
        // it is still found — no false ambiguity.
        let dir = tempfile::TempDir::new().unwrap();
        let release = dir.path().join("target/wasm32v1-none/release");
        std::fs::create_dir_all(&release).unwrap();
        let old = release.join("stale.wasm");
        std::fs::write(&old, b"x").unwrap();

        let preexisting: HashSet<PathBuf> = collect_release_wasms(dir.path()).into_iter().collect();

        // The rebuild then produces a fresh artifact.
        std::fs::write(release.join("hello.wasm"), b"x").unwrap();

        let meta = meta_with_bldopts(vec![]);
        let p = find_rebuilt_wasm(dir.path(), &meta, &preexisting).unwrap();
        assert!(p.ends_with("hello.wasm"));
    }

    #[test]
    fn source_basename_strips_url_query_and_fragment() {
        assert_eq!(
            source_basename("https://example.com/path/src.tar.gz?token=abc#frag"),
            "src.tar.gz"
        );
        assert_eq!(source_basename("https://example.com/a/b/x.tgz"), "x.tgz");
    }

    #[test]
    fn source_basename_handles_local_paths() {
        assert_eq!(source_basename("/tmp/foo/src.tar.gz"), "src.tar.gz");
        assert_eq!(source_basename("./relative/src.tgz"), "src.tgz");
        assert_eq!(source_basename("src.tar.gz"), "src.tar.gz");
    }

    #[test]
    fn resolve_source_format_accepts_recognized_extensions() {
        use source_archive::ArchiveFormat;
        assert_eq!(
            resolve_source_format("https://example.com/src.tar.gz").unwrap(),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            resolve_source_format("/tmp/src.tgz").unwrap(),
            ArchiveFormat::TarGz
        );
        assert_eq!(
            resolve_source_format("https://example.com/src.zip?token=abc").unwrap(),
            ArchiveFormat::Zip
        );
        // Case-insensitive.
        assert_eq!(
            resolve_source_format("SRC.TAR.GZ").unwrap(),
            ArchiveFormat::TarGz
        );
    }

    #[test]
    fn resolve_source_format_rejects_unknown_formats() {
        for source in [
            "https://example.com/src.rar",
            "/tmp/src.7z",
            "src",
            "src.gz",
        ] {
            let err = resolve_source_format(source).unwrap_err();
            assert!(matches!(err, Error::UnsupportedSourceFormat { .. }));
        }
    }
}
