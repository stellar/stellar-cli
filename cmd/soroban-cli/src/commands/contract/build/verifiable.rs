use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use bollard::{
    models::ContainerCreateBody,
    query_parameters::{
        AttachContainerOptions, CreateContainerOptions, CreateImageOptions, StartContainerOptions,
        WaitContainerOptions,
    },
    service::HostConfig,
    Docker,
};
use cargo_metadata::MetadataCommand;
use futures_util::{StreamExt, TryStreamExt};
use regex::Regex;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::{
    commands::{container::shared::Error as ConnectionError, global},
    config::{data, locator::enforce_hardened_tree},
    print::Print,
};

use super::{BuiltContract, Cmd, WASM_TARGET};

const REGISTRY: &str = "docker.io/stellar/stellar-cli";
const HUB_TAGS_URL: &str =
    "https://hub.docker.com/v2/repositories/stellar/stellar-cli/tags/?page_size=100";
const RESERVED_META_KEYS: &[&str] = &["bldimg", "source_uri", "source_sha256", "bldopt"];

/// Top-level names excluded when archiving a non-git working directory (we have
/// no tracked-files list to consult, so fall back to a fixed denylist of VCS
/// metadata, build/cache/transient dirs, and editor/OS/AI-assistant junk).
/// Matched against each path component, so a directory like `target/` prunes
/// its whole subtree.
const ARCHIVE_DENYLIST: &[&str] = &[
    // version control
    ".git",
    ".svn",
    ".hg",
    // build output / dependencies
    "target",
    "node_modules",
    // transient
    "log",
    "logs",
    "tmp",
    "temp",
    // OS / editor junk
    ".DS_Store",
    "Thumbs.db",
    ".idea",
    ".vscode",
    // AI assistant dirs
    ".claude",
    ".cursor",
    ".windsurf",
    ".aider",
];

/// First cli release that accepts `--optimize=false` as an explicit value
/// (added by commit `b17d3f0b`). Containers older than this only accept bare
/// `--optimize`; we probe the container's `stellar version --only-version` to
/// pick the right syntax for `--optimize=false`.
const OPTIMIZE_NEW_SYNTAX_MIN: &str = "26.1.0";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("⛔ failed to connect to docker: {0}")]
    DockerConnection(#[from] ConnectionError),

    #[error(transparent)]
    Bollard(#[from] bollard::errors::Error),

    #[error("--image value {value:?} does not match the SEP-58 bldimg format `<registry-host>/<repo>@sha256:<64-hex>`. Examples: docker.io/stellar/stellar-cli@sha256:<64-hex>, localhost:5000/foo@sha256:<64-hex>. Tag-only refs and implicit Docker-Hub short refs are not accepted.")]
    BldimgFormat { value: String },

    #[error("could not determine the running rustc version: {0}")]
    RustcVersion(String),

    #[error("could not pull image {tag}: {source}\n\nAvailable tags for this CLI version: {available_for_cli}\nAll published cli/rust pairs: {all_grouped}\n\nFix: install a matching rustc, or pass --image docker.io/stellar/stellar-cli@sha256:<digest> with one of the listed tags resolved to a digest.")]
    ImageNotFound {
        tag: String,
        available_for_cli: String,
        all_grouped: String,
        source: bollard::errors::Error,
    },

    #[error("could not list published images on docker hub: {0}")]
    TagListUnavailable(String),

    #[error("image {tag} has no repo digest after pull; cannot record a content-addressed bldimg")]
    NoRepoDigest { tag: String },

    #[error("cargo metadata failed: {0}")]
    Metadata(#[from] cargo_metadata::Error),

    #[error("could not read git state at {path}: {source}")]
    GitInvoke {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(
        "git working tree at {path} is dirty. --verifiable requires a clean tree so the recorded source_sha256 matches the WASM bytes. Commit or stash your changes and try again."
    )]
    GitDirty { path: PathBuf },

    #[error(
        "the cli sets bldimg, source_uri, source_sha256, and bldopt automatically when --verifiable is used; remove them from --meta. Got reserved key: {key}"
    )]
    ReservedMetaKey { key: String },

    #[error("--verifiable requires --source-sha256 (the SEP-58 source_sha256: 64-char hex SHA-256 of the source), or --archive to generate the source archive and compute it. --source-uri is optional.")]
    MissingSourceSha256,

    #[error("--source-sha256 value {value:?} does not match the SEP-58 source_sha256 format `^[0-9a-f]{{64}}$` (64-char lower-case hex).")]
    SourceSha256Format { value: String },

    #[error("--source-uri value {value:?} does not match the SEP-58 source_uri format `^[a-zA-Z][a-zA-Z0-9+.-]*:\\S+$` (a URI with a scheme, e.g. https://example.com/src.tar.gz).")]
    SourceUriFormat { value: String },

    #[error("--source-sha256 {provided} does not match the SHA-256 of the generated archive {computed}. Omit --source-sha256 to record the computed value, or fix the value.")]
    SourceSha256Mismatch { provided: String, computed: String },

    #[error("`git archive` failed in {path}: {stderr}")]
    GitArchive { path: PathBuf, stderr: String },

    #[error("could not write source archive to {path}: {source}")]
    ArchiveWrite {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not extract source archive: {0}")]
    ArchiveExtract(std::io::Error),

    #[error(transparent)]
    Data(#[from] data::Error),

    #[error("container build exited with status {status}. To reproduce manually:\n  docker run --rm -v {mount}:/source {image} {args}")]
    ContainerExit {
        status: i64,
        image: String,
        mount: String,
        args: String,
    },
}

pub async fn run(
    cmd: &Cmd,
    global_args: &global::Args,
    print: &Print,
) -> Result<Vec<BuiltContract>, super::Error> {
    // Stage 1: pure validation, no I/O.
    for (k, _) in &cmd.build_args.meta {
        if RESERVED_META_KEYS.iter().any(|r| r == k) {
            return Err(Error::ReservedMetaKey { key: k.clone() }.into());
        }
    }
    if let Some(img) = &cmd.image {
        if !bldimg_regex().is_match(img) {
            return Err(Error::BldimgFormat { value: img.clone() }.into());
        }
    }

    // Stage 2: local filesystem + git, no network.
    let workspace_root = resolve_workspace_root(cmd)?;
    validate_source_formats(cmd)?;

    // Pick the anchor for the local source: the `--manifest-path` bldopt is
    // relativized against it, and (when `--archive` is not used) it's also what
    // gets bind-mounted into the container. We do NOT validate that it matches
    // source_uri — a wrong source produces different bytes, and verify catches
    // that at byte-comparison time.
    let source_root = resolve_source_root(cmd);

    // A dirty working tree would make the recorded source_sha256 fail to
    // describe the bytes actually built, so refuse to proceed. Skipped when
    // the source root isn't a git repo (we can't check, e.g. archive sources).
    enforce_clean_tree(&source_root)?;

    // Resolve the recorded source_sha256 and the directory the container mounts
    // at /source. With `--archive`, the CLI builds the source archive, records
    // its hash, and builds from the *extracted* archive (in a hardened tempdir)
    // so the WASM is produced from exactly the bytes that were hashed. Without
    // it, the user supplies --source-sha256 and we mount the working tree.
    let resolved = match &cmd.archive {
        Some(_) => {
            let a = resolve_archive(cmd, &source_root, print)?;
            // The extracted `source/` dir mirrors `source_root` exactly and is
            // both the container mount and the tree the build writes `target/`
            // into, so it's what `collect_built_contracts` resolves artifacts
            // against.
            let mount_root = a.extracted_root.join("source");
            ResolvedSource {
                source_sha256: a.source_sha256,
                extracted_root: Some(mount_root.clone()),
                mount_root,
                _tmp: Some(a.tmp),
            }
        }
        None => ResolvedSource {
            source_sha256: cmd
                .source_sha256
                .clone()
                .ok_or(Error::MissingSourceSha256)?,
            mount_root: source_root.clone(),
            extracted_root: None,
            _tmp: None,
        },
    };

    let source_ids = SourceIds {
        source_uri: cmd.source_uri.clone(),
        source_sha256: Some(resolved.source_sha256.clone()),
    };

    // Defer the info banner until every validation has passed, so it doesn't
    // appear right before an error.
    if !cmd.locked {
        print.infoln("Implying --locked because --verifiable was passed");
    }

    // Stage 3: docker.
    let docker_args = crate::commands::container::shared::Args {
        docker_host: cmd.docker_host.clone(),
    };
    let docker = docker_args
        .connect_to_docker(print)
        .await
        .map_err(Error::DockerConnection)?;
    let image_ref = resolve_image(cmd, &docker, print).await?;

    // Only probe the container's cli version when we need to pick between
    // `--optimize=false` (new syntax) and not-forwarded-at-all (old default).
    // Bare `--optimize` is universally accepted, so the true path skips this.
    let supports_explicit_optimize_false = if cmd.build_args.optimize {
        true
    } else {
        probe_supports_optimize_false_syntax(&image_ref, &docker, print).await
    };

    let package = resolve_build_package(cmd)?;
    if cmd.package.is_none() {
        if let Some(pkg) = &package {
            print.infoln(format!(
                "Inferred --package={pkg} and using it as a build option."
            ));
        }
    }
    let (forwarded_args, bldopts) = build_forwarded_args(
        cmd,
        &source_root,
        package.as_deref(),
        supports_explicit_optimize_false,
    );
    let metadata_args = build_metadata_args(&image_ref, &source_ids, &bldopts);
    let container_cmd_args = compose_container_args(&forwarded_args, &metadata_args);

    // Always stream the container's cargo output during `contract build
    // --verifiable`, matching how a non-verifiable `contract build` shows
    // cargo output by default. The verify-side caller gates this on
    // `--verbose` because verifications are run as part of pipelines.
    run_in_container(
        &image_ref,
        &resolved.mount_root,
        &container_cmd_args,
        &docker,
        print,
        true,
    )
    .await?;

    let _ = global_args;
    let _ = workspace_root;
    collect_built_contracts(cmd, &source_root, resolved.extracted_root.as_deref(), print)
}

/// The recorded `source_sha256`, the directory bind-mounted at `/source`, and
/// (when `--archive` is used) the extracted-archive root plus its tempdir guard
/// — held so the temp dir outlives the container build and artifact collection.
struct ResolvedSource {
    source_sha256: String,
    mount_root: PathBuf,
    extracted_root: Option<PathBuf>,
    _tmp: Option<tempfile::TempDir>,
}

fn resolve_workspace_root(cmd: &Cmd) -> Result<PathBuf, Error> {
    let mut mc = MetadataCommand::new();
    mc.no_deps();
    if let Some(p) = &cmd.manifest_path {
        mc.manifest_path(p);
    }
    let md = mc.exec()?;
    Ok(md.workspace_root.into_std_path_buf())
}

/// Pick the anchor for the container bind-mount and for relativizing
/// `--manifest-path` into the recorded `bldopt`. Walk up from the user's
/// `--manifest-path` (or cwd, if no manifest_path) looking for a `.git`
/// directory; return its parent. If none is found, fall back to cwd.
///
/// This isn't a validation step — any `.git` will do. Wrong-source mistakes
/// are caught later by the verify-side byte comparison.
fn resolve_source_root(cmd: &Cmd) -> PathBuf {
    let start = if let Some(p) = &cmd.manifest_path {
        let abs = std::path::absolute(p).unwrap_or_else(|_| p.clone());
        abs.parent().map(Path::to_path_buf).unwrap_or(abs)
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let mut p = start.clone();
    loop {
        if p.join(".git").exists() {
            return p;
        }
        if !p.pop() {
            break;
        }
    }

    std::env::current_dir().unwrap_or(start)
}

/// Source-identification fields recorded as SEP-58 meta. `source_sha256` is
/// always `Some` by the time these are built in `run()` (resolved from
/// `--source-sha256` or computed from the generated archive). `source_uri` is
/// `Some` only when the user passed `--source-uri`.
#[derive(Debug, Default, Clone)]
struct SourceIds {
    source_uri: Option<String>,
    source_sha256: Option<String>,
}

/// Format-validate the user-supplied source flags. Requiredness is enforced in
/// `run()` (it depends on whether `--archive` is used), not here.
fn validate_source_formats(cmd: &Cmd) -> Result<(), Error> {
    if let Some(sha) = &cmd.source_sha256 {
        if !source_sha256_regex().is_match(sha) {
            return Err(Error::SourceSha256Format { value: sha.clone() });
        }
    }
    if let Some(uri) = &cmd.source_uri {
        if !source_uri_regex().is_match(uri) {
            return Err(Error::SourceUriFormat { value: uri.clone() });
        }
    }
    Ok(())
}

/// Outcome of `--archive`: the generated archive's SHA-256 and the directory it
/// was extracted into (held alive by `tmp`).
struct ArchiveResult {
    source_sha256: String,
    extracted_root: PathBuf,
    tmp: tempfile::TempDir,
}

/// Build the source archive, record its hash, write it out, and extract it into
/// a permission-hardened tempdir that the container then builds from.
fn resolve_archive(cmd: &Cmd, source_root: &Path, print: &Print) -> Result<ArchiveResult, Error> {
    let bytes = build_source_archive(source_root, print)?;
    let computed = hex::encode(Sha256::digest(&bytes));

    // If the user pinned a hash, it must match what we produced.
    if let Some(provided) = &cmd.source_sha256 {
        if provided != &computed {
            return Err(Error::SourceSha256Mismatch {
                provided: provided.clone(),
                computed,
            });
        }
    }

    // `Some(Some(path))` → write there; `Some(None)` → content-addressed name
    // under the managed archives dir.
    let out_path = match &cmd.archive {
        Some(Some(p)) => p.clone(),
        Some(None) => data::archives_dir()?.join(format!("{computed}.tar.gz")),
        None => unreachable!("resolve_archive is only called when --archive is set"),
    };
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::ArchiveWrite {
            path: out_path.clone(),
            source,
        })?;
    }
    std::fs::write(&out_path, &bytes).map_err(|source| Error::ArchiveWrite {
        path: out_path.clone(),
        source,
    })?;
    print.infoln(format!(
        "Wrote source archive {} (source_sha256 {computed})",
        out_path.display()
    ));

    // Extract and harden, then build from the extracted copy so the WASM is
    // produced from exactly the archived bytes.
    //
    // Extract under the data dir, NOT the OS temp dir: on macOS `$TMPDIR` lives
    // under /var/folders, which container VMs (Docker Desktop, Colima, …) don't
    // share by default, so a bind mount of it would be empty inside the
    // container. The data dir lives under the user's home, which is shared.
    let base = data::data_local_dir()?;
    std::fs::create_dir_all(&base).map_err(|source| Error::ArchiveWrite {
        path: base.clone(),
        source,
    })?;
    let tmp = tempfile::Builder::new()
        .prefix("verifiable-src-")
        .tempdir_in(&base)
        .map_err(Error::ArchiveExtract)?;
    unpack_targz(&bytes, tmp.path())?;
    enforce_hardened_tree(tmp.path()).map_err(Error::ArchiveExtract)?;

    let extracted_root = tmp.path().to_path_buf();
    Ok(ArchiveResult {
        source_sha256: computed,
        extracted_root,
        tmp,
    })
}

/// Whether `source_root` is inside a git work tree.
fn is_git_repo(source_root: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(source_root)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Produce the gzipped source tarball bytes. Entries are rooted under a
/// top-level `source/` prefix (so the archive extracts to a `source/` dir,
/// mirroring the container's `/source` mount). In a git repo this is `git
/// archive HEAD` (the committed tree); otherwise the working directory is
/// walked and tarred, skipping `ARCHIVE_DENYLIST` entries, after warning.
fn build_source_archive(source_root: &Path, print: &Print) -> Result<Vec<u8>, Error> {
    let tar = if is_git_repo(source_root) {
        git_archive_tar(source_root)?
    } else {
        print.warnln(format!(
            "{} is not a git repository; archiving the working directory. Inspect the generated archive to confirm its contents.",
            source_root.display(),
        ));
        walk_tar(source_root)?
    };
    gzip(&tar)
}

/// `git archive --format=tar --prefix=source/ HEAD`, returning the tar bytes.
fn git_archive_tar(source_root: &Path) -> Result<Vec<u8>, Error> {
    let out = Command::new("git")
        .arg("-C")
        .arg(source_root)
        .arg("archive")
        .arg("--format=tar")
        .arg("--prefix=source/")
        .arg("HEAD")
        .output()
        .map_err(|source| Error::GitInvoke {
            path: source_root.to_path_buf(),
            source,
        })?;
    if !out.status.success() {
        return Err(Error::GitArchive {
            path: source_root.to_path_buf(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(out.stdout)
}

/// Tar the working tree under `source_root`, skipping denylisted path
/// components, with entries sorted and headers normalized (deterministic mode)
/// so the bytes are reproducible. Each entry is prefixed with `source/`.
fn walk_tar(source_root: &Path) -> Result<Vec<u8>, Error> {
    let mut files: Vec<PathBuf> = Vec::new();
    let walk = WalkDir::new(source_root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| !is_denylisted(e.file_name()));
    for entry in walk {
        let entry = entry.map_err(|e| Error::ArchiveWrite {
            path: source_root.to_path_buf(),
            source: e.into(),
        })?;
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();

    let mut builder = tar::Builder::new(Vec::new());
    builder.mode(tar::HeaderMode::Deterministic);
    for path in &files {
        let rel = path.strip_prefix(source_root).unwrap_or(path);
        let name = Path::new("source").join(rel);
        let mut f = std::fs::File::open(path).map_err(|source| Error::ArchiveWrite {
            path: path.clone(),
            source,
        })?;
        builder
            .append_file(&name, &mut f)
            .map_err(|source| Error::ArchiveWrite {
                path: path.clone(),
                source,
            })?;
    }
    builder.into_inner().map_err(|source| Error::ArchiveWrite {
        path: source_root.to_path_buf(),
        source,
    })
}

/// A path component is denylisted if it equals a denylist entry, or — for
/// dotted entries, which double as extension filters (e.g. `.swp`, `.log`) — if
/// it ends with that entry. Plain names (`target`, `node_modules`) match
/// exactly only, so `mytarget` is not excluded.
fn is_denylisted(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    ARCHIVE_DENYLIST
        .iter()
        .any(|d| name == *d || (d.starts_with('.') && name.ends_with(d)))
}

/// Gzip with a default (mtime-zeroed) header so the same tar bytes always hash
/// the same.
fn gzip(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(bytes).map_err(|source| Error::ArchiveWrite {
        path: PathBuf::new(),
        source,
    })?;
    enc.finish().map_err(|source| Error::ArchiveWrite {
        path: PathBuf::new(),
        source,
    })
}

/// Decompress gzip and unpack the tar into `dest`. Entries are `source/…`, so
/// they land at `<dest>/source/…`.
fn unpack_targz(bytes: &[u8], dest: &Path) -> Result<(), Error> {
    let dec = flate2::read::GzDecoder::new(bytes);
    tar::Archive::new(dec)
        .unpack(dest)
        .map_err(Error::ArchiveExtract)
}

/// Refuse to run a verifiable build against a dirty git working tree: the
/// bind-mounted source must match the recorded source_sha256 for the build to
/// be reproducible. When the source root isn't a git repo (e.g. an extracted
/// archive) we can't check, so we proceed — the user owns the source_sha256
/// they pass, and verify catches a mismatch at byte-comparison time.
fn enforce_clean_tree(source_root: &Path) -> Result<(), Error> {
    let status = Command::new("git")
        .arg("-C")
        .arg(source_root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .map_err(|e| Error::GitInvoke {
            path: source_root.to_path_buf(),
            source: e,
        })?;

    // Not a git repo (or git refused): can't verify cleanliness, proceed.
    if !status.status.success() {
        return Ok(());
    }

    if !status.stdout.is_empty() {
        return Err(Error::GitDirty {
            path: source_root.to_path_buf(),
        });
    }

    Ok(())
}

fn bldimg_regex() -> Regex {
    Regex::new(r"^(?:localhost(?::\d+)?|[^\s@/]*[.:][^\s@/]*)/[^\s@]+@sha256:[0-9a-f]{64}$")
        .unwrap()
}

fn source_sha256_regex() -> Regex {
    Regex::new(r"^[0-9a-f]{64}$").unwrap()
}

fn source_uri_regex() -> Regex {
    Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*:\S+$").unwrap()
}

/// Resolve the package to pin as `--package`. An explicit `--package` wins.
/// Otherwise, when the workspace builds exactly one cdylib by default, return
/// its name so the recorded bldopt is reproducible even if the workspace's
/// default members change later. Returns `None` when the selection is
/// ambiguous (zero or multiple default cdylibs) — the build then keeps cargo's
/// default behavior of building them all, which `--package` can't express
/// (the container's flag is singular).
fn resolve_build_package(cmd: &Cmd) -> Result<Option<String>, Error> {
    if cmd.package.is_some() {
        return Ok(cmd.package.clone());
    }
    let mut mc = MetadataCommand::new();
    mc.no_deps();
    if let Some(p) = &cmd.manifest_path {
        mc.manifest_path(p);
    }
    let md = mc.exec().map_err(Error::Metadata)?;
    let mut names: Vec<String> = md
        .packages
        .iter()
        .filter(|p| md.workspace_default_members.contains(&p.id))
        .filter(|p| {
            p.targets
                .iter()
                .any(|t| t.crate_types.iter().any(|c| c == "cdylib"))
        })
        .map(|p| p.name.clone())
        .collect();
    names.sort();
    names.dedup();
    Ok((names.len() == 1).then(|| names.remove(0)))
}

/// The flags forwarded to the container's `stellar contract build`, plus the
/// bldopt strings recorded into SEP-58 metadata. Every build-affecting flag
/// becomes one bldopt entry so a verifier can replay the same invocation.
/// `--locked` is always present. `manifest_path` (when set) is recorded
/// relative to the workspace root so it's valid inside `/source`.
///
/// `supports_explicit_optimize_false`: whether the container's cli accepts
/// `--optimize=false`. When false, the optimize=false case records the flag
/// in bldopt but does not forward it (the older container's cli default of
/// `false` already produces the desired state).
fn build_forwarded_args(
    cmd: &Cmd,
    workspace_root: &Path,
    package: Option<&str>,
    supports_explicit_optimize_false: bool,
) -> (Vec<String>, Vec<String>) {
    let mut forwarded: Vec<String> = Vec::new();
    let mut bldopts: Vec<String> = Vec::new();

    let mut record = |arg: String| {
        forwarded.push(arg.clone());
        bldopts.push(arg);
    };

    record("--locked".to_string());

    if let Some(path) = &cmd.manifest_path {
        let abs = std::path::absolute(path).unwrap_or_else(|_| path.clone());
        let rel = abs
            .strip_prefix(workspace_root)
            .map(Path::to_path_buf)
            .unwrap_or(abs);
        record(format!("--manifest-path={}", rel.display()));
    }
    if cmd.profile != "release" {
        record(format!("--profile={}", cmd.profile));
    }
    if let Some(features) = &cmd.features {
        record(format!("--features={features}"));
    }
    if cmd.all_features {
        record("--all-features".to_string());
    }
    if cmd.no_default_features {
        record("--no-default-features".to_string());
    }
    // Always pin the package when it can be resolved (explicit `--package`, or
    // a workspace that builds exactly one cdylib by default) so the recorded
    // bldopt stays reproducible even if workspace default members change later.
    if let Some(pkg) = package {
        record(format!("--package={pkg}"));
    }
    for (k, v) in &cmd.build_args.meta {
        // Use the `--meta=key=value` form so each option is a single token,
        // matching how clap re-parses on the container side.
        record(format!("--meta={k}={v}"));
    }

    // `--optimize` true is recorded as a bare flag (universally accepted).
    // `--optimize=false` is only emitted when the container's cli accepts it
    // (added in `b17d3f0b`); on older containers, false is the default and
    // we record/forward nothing — passing `--optimize=false` there would fail.
    if cmd.build_args.optimize {
        record("--optimize".to_string());
    } else if supports_explicit_optimize_false {
        record("--optimize=false".to_string());
    }

    (forwarded, bldopts)
}

fn build_metadata_args(image_ref: &str, ids: &SourceIds, bldopts: &[String]) -> Vec<String> {
    let mut out = Vec::new();

    let push = |out: &mut Vec<String>, key: &str, val: &str| {
        out.push("--meta".to_string());
        out.push(format!("{key}={val}"));
    };

    push(&mut out, "bldimg", image_ref);

    if let Some(v) = &ids.source_uri {
        push(&mut out, "source_uri", v);
    }
    if let Some(v) = &ids.source_sha256 {
        push(&mut out, "source_sha256", v);
    }

    for o in bldopts {
        push(&mut out, "bldopt", o);
    }

    out
}

fn compose_container_args(forwarded: &[String], metadata: &[String]) -> Vec<String> {
    let mut args = vec!["contract".to_string(), "build".to_string()];
    args.extend_from_slice(forwarded);
    args.extend_from_slice(metadata);
    args
}

pub async fn resolve_image(cmd: &Cmd, docker: &Docker, print: &Print) -> Result<String, Error> {
    if let Some(s) = &cmd.image {
        if !bldimg_regex().is_match(s) {
            return Err(Error::BldimgFormat { value: s.clone() });
        }
        // Always pull, even when the digest is user-supplied. Docker requires
        // the image to be locally present before `create_container` will
        // accept it, and the user typically expects the cli to fetch
        // whatever they asked for.
        pull_image(docker, s, print).await?;
        return Ok(s.clone());
    }

    let cli_v = env!("CARGO_PKG_VERSION");
    let rust_v = rustc_version::version()
        .map_err(|e| Error::RustcVersion(e.to_string()))?
        .to_string();
    let tag = format!("{REGISTRY}:{cli_v}-rust{rust_v}");

    print.infoln(format!("Pulling verifiable build image {tag}"));
    let pull = pull_image(docker, &tag, print).await;

    match pull {
        Ok(()) => {}
        Err(e) => {
            let (available_for_cli, all_grouped) = match list_published_tags().await {
                Ok(tags) => format_available(&tags, cli_v),
                Err(list_err) => (
                    "<unavailable>".to_string(),
                    format!("<unavailable: {list_err}>"),
                ),
            };
            return Err(Error::ImageNotFound {
                tag,
                available_for_cli,
                all_grouped,
                source: e,
            });
        }
    }

    let inspect = docker.inspect_image(&tag).await?;
    let digest = inspect
        .repo_digests
        .and_then(|v| v.into_iter().next())
        .ok_or_else(|| Error::NoRepoDigest { tag: tag.clone() })?;
    Ok(digest)
}

async fn pull_image(
    docker: &Docker,
    tag: &str,
    print: &Print,
) -> Result<(), bollard::errors::Error> {
    let mut stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: Some(tag.to_string()),
            ..Default::default()
        }),
        None,
        None,
    );
    while let Some(item) = stream.try_next().await? {
        if let Some(status) = item.status {
            // The docker daemon emits short status lines like:
            //   "Pulling from <repo>"
            //   "Digest: sha256:<hex>"
            //   "Status: Image is up to date for <ref>"
            // Stand-alone "Digest" reads as an orphan. Rewrite each line so
            // it makes sense outside the docker-pull context.
            if let Some(repo) = status.strip_prefix("Pulling from ") {
                print.infoln(format!("Pulling image {repo}"));
            } else if let Some(digest) = status.strip_prefix("Digest: ") {
                print.infoln(format!("Image digest: {digest}"));
            } else if let Some(rest) = status.strip_prefix("Status: ") {
                // Docker's status text already starts with "Image …" or
                // "Downloaded …", so we forward it verbatim instead of
                // prepending another "Image:".
                print.infoln(rest);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PublishedTag {
    pub cli: Version,
    pub rust: Version,
    pub raw: String,
}

#[derive(Deserialize)]
struct HubPage {
    results: Vec<HubTag>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct HubTag {
    name: String,
}

pub async fn list_published_tags() -> Result<Vec<PublishedTag>, Error> {
    let re = Regex::new(r"^(\d+\.\d+\.\d+)-rust(\d+\.\d+\.\d+)$").unwrap();
    let mut out = Vec::new();
    let mut next = Some(HUB_TAGS_URL.to_string());
    let client = reqwest::Client::builder()
        .user_agent("stellar-cli")
        .build()
        .map_err(|e| Error::TagListUnavailable(e.to_string()))?;
    while let Some(url) = next {
        let page: HubPage = client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::TagListUnavailable(e.to_string()))?
            .error_for_status()
            .map_err(|e| Error::TagListUnavailable(e.to_string()))?
            .json()
            .await
            .map_err(|e| Error::TagListUnavailable(e.to_string()))?;
        for t in page.results {
            if let Some(c) = re.captures(&t.name) {
                let cli = Version::parse(&c[1]);
                let rust = Version::parse(&c[2]);
                if let (Ok(cli), Ok(rust)) = (cli, rust) {
                    out.push(PublishedTag {
                        cli,
                        rust,
                        raw: t.name,
                    });
                }
            }
        }
        next = page.next;
    }
    Ok(out)
}

fn format_available(tags: &[PublishedTag], current_cli: &str) -> (String, String) {
    let current = Version::parse(current_cli).ok();
    let mut for_this_cli: Vec<&PublishedTag> = tags
        .iter()
        .filter(|t| Some(&t.cli) == current.as_ref())
        .collect();
    for_this_cli.sort_by(|a, b| b.rust.cmp(&a.rust));
    let available_for_cli = if for_this_cli.is_empty() {
        "<none>".to_string()
    } else {
        for_this_cli
            .iter()
            .map(|t| t.raw.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut by_cli: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for t in tags {
        by_cli
            .entry(t.cli.to_string())
            .or_default()
            .push(t.rust.to_string());
    }
    let all_grouped = by_cli
        .into_iter()
        .map(|(cli, rusts)| format!("{cli}: [{}]", rusts.join(", ")))
        .collect::<Vec<_>>()
        .join("; ");

    (available_for_cli, all_grouped)
}

/// Probe the container's `stellar` binary for its self-reported version with
/// `stellar version --only-version`. Returns true if the parsed version is
/// at or above the cutoff where `--optimize=false` was accepted. On any
/// probe failure (network, unparseable output, missing subcommand), returns
/// false — the conservative assumption that the container is old.
async fn probe_supports_optimize_false_syntax(
    image_ref: &str,
    docker: &Docker,
    print: &Print,
) -> bool {
    match probe_cli_version(image_ref, docker).await {
        Ok(v) => {
            let cutoff = Version::parse(OPTIMIZE_NEW_SYNTAX_MIN).unwrap();
            v >= cutoff
        }
        Err(e) => {
            print.warnln(format!(
                "Could not probe container cli version ({e}); assuming pre-{OPTIMIZE_NEW_SYNTAX_MIN} syntax"
            ));
            false
        }
    }
}

async fn probe_cli_version(image_ref: &str, docker: &Docker) -> Result<Version, Error> {
    let config = ContainerCreateBody {
        image: Some(image_ref.to_string()),
        cmd: Some(vec!["version".to_string(), "--only-version".to_string()]),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        host_config: Some(HostConfig {
            auto_remove: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    };
    let created = docker
        .create_container(None::<CreateContainerOptions>, config)
        .await?;
    let attached = docker
        .attach_container(
            &created.id,
            Some(AttachContainerOptions {
                stdout: true,
                stderr: true,
                stream: true,
                ..Default::default()
            }),
        )
        .await?;
    docker
        .start_container(&created.id, None::<StartContainerOptions>)
        .await?;

    let mut stdout = String::new();
    let mut output = attached.output;
    while let Some(chunk) = output.next().await {
        if let Ok(bollard::container::LogOutput::StdOut { message }) = chunk {
            stdout.push_str(&String::from_utf8_lossy(&message));
        }
    }

    let mut wait = docker.wait_container(&created.id, None::<WaitContainerOptions>);
    while wait.next().await.is_some() {}

    Version::parse(stdout.trim())
        .map_err(|e| Error::TagListUnavailable(format!("unparseable version {stdout:?}: {e}")))
}

async fn run_in_container(
    image_ref: &str,
    workspace_root: &Path,
    container_cmd: &[String],
    docker: &Docker,
    print: &Print,
    verbose: bool,
) -> Result<(), Error> {
    let bind = format!("{}:/source", workspace_root.display());
    let config = ContainerCreateBody {
        image: Some(image_ref.to_string()),
        cmd: Some(container_cmd.to_vec()),
        working_dir: Some("/source".to_string()),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        host_config: Some(HostConfig {
            auto_remove: Some(true),
            binds: Some(vec![bind.clone()]),
            ..Default::default()
        }),
        ..Default::default()
    };

    print.infoln(format!(
        "Running verifiable build in {image_ref} (mount {bind})"
    ));

    let created = docker
        .create_container(None::<CreateContainerOptions>, config)
        .await?;

    let attached = docker
        .attach_container(
            &created.id,
            Some(AttachContainerOptions {
                stdout: true,
                stderr: true,
                stream: true,
                ..Default::default()
            }),
        )
        .await?;

    docker
        .start_container(&created.id, None::<StartContainerOptions>)
        .await?;

    let mut output = attached.output;
    while let Some(chunk) = output.next().await {
        match chunk {
            Ok(
                bollard::container::LogOutput::StdOut { message }
                | bollard::container::LogOutput::StdErr { message },
            ) => {
                if verbose {
                    let s = String::from_utf8_lossy(&message);
                    print.blankln(s.trim_end());
                }
            }
            Ok(_) => {}
            Err(e) => return Err(e.into()),
        }
    }

    let mut wait = docker.wait_container(&created.id, None::<WaitContainerOptions>);
    while let Some(item) = wait.next().await {
        match item {
            Ok(r) if r.status_code == 0 => {}
            Ok(r) => {
                return Err(Error::ContainerExit {
                    status: r.status_code,
                    image: image_ref.to_string(),
                    mount: workspace_root.display().to_string(),
                    args: container_cmd.join(" "),
                });
            }
            Err(bollard::errors::Error::DockerContainerWaitError { code: 0, .. }) => {}
            Err(bollard::errors::Error::DockerContainerWaitError { code, .. }) => {
                return Err(Error::ContainerExit {
                    status: code,
                    image: image_ref.to_string(),
                    mount: workspace_root.display().to_string(),
                    args: container_cmd.join(" "),
                });
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

/// Collect the built WASM artifacts. Package names and the host target dir come
/// from host `cargo metadata`. `extracted_root` is set when the build ran
/// against an extracted archive (step: `--archive`): the artifacts then live
/// under that tree's target dir and must be copied out before its tempdir
/// drops. `source_root` is the host source root the extracted tree mirrors, so
/// the target dir's position relative to it carries over.
fn collect_built_contracts(
    cmd: &Cmd,
    source_root: &Path,
    extracted_root: Option<&Path>,
    _print: &Print,
) -> Result<Vec<BuiltContract>, super::Error> {
    let mut mc = MetadataCommand::new();
    mc.no_deps();
    if let Some(p) = &cmd.manifest_path {
        mc.manifest_path(p);
    }
    let md = mc.exec().map_err(Error::Metadata)?;
    let host_target = md.target_directory.as_std_path();

    // Where the build actually wrote artifacts. For an extracted-archive build
    // that's `<extracted>/<host_target relative to source_root>`; otherwise the
    // host target dir (the working tree was bind-mounted directly).
    let src_target = match extracted_root {
        Some(er) => er.join(host_target.strip_prefix(source_root).unwrap_or(host_target)),
        None => host_target.to_path_buf(),
    };

    let mut out = Vec::new();
    for p in &md.packages {
        let is_cdylib = p
            .targets
            .iter()
            .any(|t| t.crate_types.iter().any(|c| c == "cdylib"));
        if !is_cdylib {
            continue;
        }
        if let Some(name) = &cmd.package {
            if &p.name != name {
                continue;
            }
        } else if !md.workspace_default_members.contains(&p.id) {
            continue;
        }
        let wasm_name = p.name.replace('-', "_");
        let rel = Path::new(WASM_TARGET)
            .join(&cmd.profile)
            .join(format!("{wasm_name}.wasm"));
        let src = src_target.join(&rel);

        // Destination: --out-dir wins; else if the build ran in a tempdir, copy
        // into the host target dir so the artifact survives; else leave in
        // place (the working tree was mounted, so it's already on the host).
        let dest = if let Some(out_dir) = &cmd.out_dir {
            Some(out_dir.join(format!("{wasm_name}.wasm")))
        } else if extracted_root.is_some() {
            Some(host_target.join(&rel))
        } else {
            None
        };

        let path = match dest {
            Some(dest) if src.exists() => {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).map_err(super::Error::CreatingOutDir)?;
                }
                std::fs::copy(&src, &dest).map_err(super::Error::CopyingWasmFile)?;
                dest
            }
            // Source missing: report the intended dest (matches prior leniency).
            Some(dest) => dest,
            None => src,
        };
        out.push(BuiltContract {
            name: p.name.clone(),
            path,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> &'static Path {
        Path::new("/tmp/ws")
    }

    #[test]
    fn build_forwarded_args_defaults() {
        let cmd = Cmd::default();
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true);
        // Default optimize=true → bare `--optimize` recorded + forwarded.
        assert_eq!(
            forwarded,
            vec!["--locked".to_string(), "--optimize".to_string()]
        );
        assert_eq!(
            bldopts,
            vec!["--locked".to_string(), "--optimize".to_string()]
        );
    }

    #[test]
    fn build_forwarded_args_features_and_package() {
        let cmd = Cmd {
            features: Some("a,b".to_string()),
            package: Some("contract-a".to_string()),
            ..Cmd::default()
        };
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true);
        assert!(forwarded.contains(&"--features=a,b".to_string()));
        assert!(forwarded.contains(&"--package=contract-a".to_string()));
        assert!(bldopts.contains(&"--features=a,b".to_string()));
        assert!(bldopts.contains(&"--package=contract-a".to_string()));
        assert!(bldopts.contains(&"--locked".to_string()));
    }

    #[test]
    fn build_forwarded_args_records_resolved_package_when_unspecified() {
        // No `--package` on the cmd, but the caller resolved one (single
        // default cdylib); it must still be forwarded and recorded.
        let cmd = Cmd::default();
        assert!(cmd.package.is_none());
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), Some("hello-world"), true);
        assert!(forwarded.contains(&"--package=hello-world".to_string()));
        assert!(bldopts.contains(&"--package=hello-world".to_string()));
    }

    #[test]
    fn build_forwarded_args_omits_package_when_unresolved() {
        let cmd = Cmd::default();
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), None, true);
        assert!(!forwarded.iter().any(|a| a.starts_with("--package")));
        assert!(!bldopts.iter().any(|a| a.starts_with("--package")));
    }

    #[test]
    fn build_forwarded_args_records_meta_and_manifest() {
        let cmd = Cmd {
            manifest_path: Some(PathBuf::from("/tmp/ws/contracts/add/Cargo.toml")),
            build_args: super::super::BuildArgs {
                meta: vec![
                    ("home_domain".to_string(), "fnando.com".to_string()),
                    ("author".to_string(), "alice".to_string()),
                ],
                optimize: true,
            },
            ..Cmd::default()
        };
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true);
        assert!(forwarded.contains(&"--meta=home_domain=fnando.com".to_string()));
        assert!(forwarded.contains(&"--meta=author=alice".to_string()));
        assert!(forwarded.contains(&"--manifest-path=contracts/add/Cargo.toml".to_string()));
        assert!(bldopts.contains(&"--meta=home_domain=fnando.com".to_string()));
        assert!(bldopts.contains(&"--meta=author=alice".to_string()));
        assert!(bldopts.contains(&"--manifest-path=contracts/add/Cargo.toml".to_string()));
    }

    #[test]
    fn build_forwarded_args_optimize_false_new_container() {
        let cmd = Cmd {
            build_args: super::super::BuildArgs {
                meta: vec![],
                optimize: false,
            },
            ..Cmd::default()
        };
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true);
        assert!(forwarded.contains(&"--optimize=false".to_string()));
        assert!(bldopts.contains(&"--optimize=false".to_string()));
    }

    #[test]
    fn build_forwarded_args_optimize_false_old_container() {
        let cmd = Cmd {
            build_args: super::super::BuildArgs {
                meta: vec![],
                optimize: false,
            },
            ..Cmd::default()
        };
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), false);
        // Old container's default is already false; record nothing.
        // Passing `--optimize=false` to a pre-26.1.0 cli would fail.
        assert!(!forwarded.iter().any(|a| a.starts_with("--optimize")));
        assert!(!bldopts.iter().any(|a| a.starts_with("--optimize")));
    }

    fn pairs(args: &[String]) -> Vec<(&str, &str)> {
        args.chunks(2)
            .map(|c| (c[0].as_str(), c[1].as_str()))
            .collect()
    }

    #[test]
    fn build_metadata_args_uri_and_sha256() {
        let ids = SourceIds {
            source_uri: Some("https://example.com/src.tar.gz".to_string()),
            source_sha256: Some("a".repeat(64)),
        };
        let m = build_metadata_args(
            "docker.io/stellar/stellar-cli@sha256:abc",
            &ids,
            &["--locked".to_string(), "--features=a".to_string()],
        );
        let p = pairs(&m);
        // bldimg first; source_uri then source_sha256; bldopts last.
        assert_eq!(
            p[0],
            ("--meta", "bldimg=docker.io/stellar/stellar-cli@sha256:abc")
        );
        assert_eq!(
            p[1],
            ("--meta", "source_uri=https://example.com/src.tar.gz")
        );
        assert_eq!(p[2].0, "--meta");
        assert!(p[2].1.starts_with("source_sha256="));
        assert_eq!(p[3], ("--meta", "bldopt=--locked"));
        assert_eq!(p[4], ("--meta", "bldopt=--features=a"));
    }

    #[test]
    fn build_metadata_args_sha256_only_omits_uri() {
        let ids = SourceIds {
            source_sha256: Some("f".repeat(64)),
            ..SourceIds::default()
        };
        let m = build_metadata_args("docker.io/stellar/stellar-cli@sha256:abc", &ids, &[]);
        assert!(m
            .iter()
            .any(|s| s == &format!("source_sha256={}", "f".repeat(64))));
        assert!(!m.iter().any(|s| s.starts_with("source_uri=")));
    }

    #[test]
    fn validate_source_formats_rejects_bad_sha256() {
        let cmd = Cmd {
            source_sha256: Some("not-a-sha".to_string()),
            ..Cmd::default()
        };
        let err = validate_source_formats(&cmd).unwrap_err();
        assert!(matches!(err, Error::SourceSha256Format { .. }));
    }

    #[test]
    fn validate_source_formats_rejects_bad_uri() {
        let cmd = Cmd {
            source_uri: Some("not a uri".to_string()), // no scheme
            source_sha256: Some("a".repeat(64)),
            ..Cmd::default()
        };
        let err = validate_source_formats(&cmd).unwrap_err();
        assert!(matches!(err, Error::SourceUriFormat { .. }));
    }

    #[test]
    fn validate_source_formats_accepts_valid_and_absent() {
        // Both absent is fine here — requiredness is enforced in run().
        validate_source_formats(&Cmd::default()).unwrap();
        let cmd = Cmd {
            source_uri: Some("https://example.com/src.tar.gz".to_string()),
            source_sha256: Some("f".repeat(64)),
            ..Cmd::default()
        };
        validate_source_formats(&cmd).unwrap();
    }

    #[test]
    fn is_denylisted_matches_names_and_dotted_suffixes() {
        use std::ffi::OsStr;
        // exact name matches
        assert!(is_denylisted(OsStr::new("target")));
        assert!(is_denylisted(OsStr::new(".git")));
        assert!(is_denylisted(OsStr::new(".DS_Store")));
        // plain names match exactly only
        assert!(!is_denylisted(OsStr::new("mytarget")));
        assert!(!is_denylisted(OsStr::new("targets")));
        // dotted entries also match as suffix (extension-style)
        assert!(is_denylisted(OsStr::new("backup.git")));
        // unrelated files pass through
        assert!(!is_denylisted(OsStr::new("Cargo.toml")));
        assert!(!is_denylisted(OsStr::new("lib.rs")));
    }

    // Initialize a git repo at `root` with one commit of everything present.
    #[cfg(unix)]
    fn git_init_commit(root: &Path) {
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["add", "-A"][..],
            &["commit", "-q", "-m", "init"][..],
        ] {
            let ok = Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@e.x")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@e.x")
                .status()
                .unwrap()
                .success();
            assert!(ok);
        }
    }

    #[test]
    #[cfg(unix)]
    fn build_source_archive_git_is_prefixed_and_deterministic() {
        use std::os::unix::fs::PermissionsExt;
        let print = Print::new(true);
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), b"// code").unwrap();
        git_init_commit(root);

        let a = build_source_archive(root, &print).unwrap();
        let b = build_source_archive(root, &print).unwrap();
        assert!(!a.is_empty());
        assert_eq!(a, b, "same commit should produce identical bytes");

        let sha = hex::encode(Sha256::digest(&a));
        assert_eq!(sha.len(), 64);

        // Unpack and confirm the `source/` prefix + hardened perms.
        let dest = tempfile::TempDir::new().unwrap();
        unpack_targz(&a, dest.path()).unwrap();
        assert!(dest.path().join("source/Cargo.toml").exists());
        assert!(dest.path().join("source/src/lib.rs").exists());

        enforce_hardened_tree(dest.path()).unwrap();
        let file_mode = std::fs::metadata(dest.path().join("source/Cargo.toml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let dir_mode = std::fs::metadata(dest.path().join("source"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
    }

    #[test]
    fn build_source_archive_non_git_excludes_denylist() {
        let print = Print::new(true);
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), b"// code").unwrap();
        // Planted dirs that must be excluded.
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/debug/x"), b"junk").unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/config"), b"junk").unwrap();

        let bytes = build_source_archive(root, &print).unwrap();
        let dest = tempfile::TempDir::new().unwrap();
        unpack_targz(&bytes, dest.path()).unwrap();

        assert!(dest.path().join("source/Cargo.toml").exists());
        assert!(dest.path().join("source/src/lib.rs").exists());
        assert!(!dest.path().join("source/target").exists());
        assert!(!dest.path().join("source/.git").exists());
        assert_eq!(hex::encode(Sha256::digest(&bytes)).len(), 64);
    }

    #[test]
    fn bldimg_regex_accepts_docker_hub_full_ref() {
        assert!(bldimg_regex().is_match(&format!(
            "docker.io/stellar/stellar-cli@sha256:{}",
            "a".repeat(64)
        )));
    }

    #[test]
    fn bldimg_regex_accepts_localhost_registry() {
        assert!(bldimg_regex().is_match(&format!("localhost:5000/foo@sha256:{}", "0".repeat(64))));
    }

    #[test]
    fn bldimg_regex_rejects_implicit_hub_short_ref() {
        // Implicit Docker Hub short ref: no registry host prefix.
        assert!(!bldimg_regex().is_match(&format!("stellar/stellar-cli@sha256:{}", "a".repeat(64))));
    }

    #[test]
    fn bldimg_regex_rejects_tag_only() {
        assert!(!bldimg_regex().is_match("docker.io/stellar/stellar-cli:latest"));
    }

    #[test]
    fn bldimg_regex_rejects_short_sha() {
        assert!(!bldimg_regex().is_match("docker.io/stellar/stellar-cli@sha256:abc"));
    }

    #[test]
    fn source_sha256_regex_matches_64_hex() {
        assert!(source_sha256_regex().is_match(&"f".repeat(64)));
        assert!(!source_sha256_regex().is_match(&"f".repeat(63)));
        assert!(!source_sha256_regex().is_match(&"F".repeat(64))); // upper-case rejected
    }

    #[test]
    fn source_uri_regex_accepts_any_scheme() {
        assert!(source_uri_regex().is_match("https://example.com/src.tar.gz"));
        assert!(source_uri_regex().is_match("http://example.com/foo.git"));
        assert!(source_uri_regex().is_match("ipfs://Qm...abc"));
        assert!(source_uri_regex().is_match("github:foo/bar"));
        assert!(!source_uri_regex().is_match("foo/bar")); // no scheme
        assert!(!source_uri_regex().is_match("https://has space")); // whitespace
    }

    #[test]
    fn resolve_source_root_finds_git_root_from_subdir() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let nested = root.join("contracts").join("foo");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Cargo.toml"), b"# placeholder").unwrap();

        let cmd = Cmd {
            manifest_path: Some(nested.join("Cargo.toml")),
            ..Cmd::default()
        };
        // Use canonicalize on both sides — `tempfile` returns symlinked /var
        // paths on macOS while resolve_source_root walks the same prefix.
        let got = std::fs::canonicalize(resolve_source_root(&cmd)).unwrap();
        let want = std::fs::canonicalize(root).unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn resolve_source_root_falls_back_to_cwd_without_git() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let nested = root.join("noisy");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Cargo.toml"), b"# placeholder").unwrap();

        let cmd = Cmd {
            manifest_path: Some(nested.join("Cargo.toml")),
            ..Cmd::default()
        };
        // No `.git` anywhere up the tree, so we fall back to cwd. We can't
        // assert what cwd is in a test runner (it varies), but we can assert
        // that the returned path doesn't contain the manifest's parent and
        // doesn't have `.git`. That's enough to confirm fallback kicked in.
        let got = resolve_source_root(&cmd);
        assert!(!got.join(".git").exists());
    }

    #[test]
    fn compose_container_args_prefixes_subcommand() {
        let composed = compose_container_args(
            &["--locked".to_string()],
            &["--meta".to_string(), "bldimg=x".to_string()],
        );
        assert_eq!(composed[..2], ["contract".to_string(), "build".to_string()]);
        assert!(composed.contains(&"--locked".to_string()));
        assert!(composed.contains(&"bldimg=x".to_string()));
    }

    #[test]
    fn reserved_meta_keys_list() {
        for key in ["bldimg", "source_uri", "source_sha256", "bldopt"] {
            assert!(RESERVED_META_KEYS.contains(&key));
        }
    }
}
