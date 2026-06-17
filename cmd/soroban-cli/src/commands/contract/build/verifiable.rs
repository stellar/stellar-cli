use std::path::{Path, PathBuf};
use std::process::Stdio;

use cargo_metadata::MetadataCommand;
use regex::Regex;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{
    commands::{
        container::shared::{self, Error as ConnectionError},
        global,
    },
    config::data,
    print::Print,
};

use super::{source_archive, BuiltContract, Cmd, WASM_TARGET};

const REGISTRY: &str = "docker.io/stellar/stellar-cli";
const HUB_TAGS_URL: &str =
    "https://hub.docker.com/v2/repositories/stellar/stellar-cli/tags/?page_size=100";
const RESERVED_META_KEYS: &[&str] = &["bldimg", "source_uri", "source_sha256", "bldopt"];

/// First cli release that accepts `--optimize=false` as an explicit value
/// (added by commit `b17d3f0b`). Containers older than this only accept bare
/// `--optimize`; we probe the container's `stellar version --only-version` to
/// pick the right syntax for `--optimize=false`.
const OPTIMIZE_NEW_SYNTAX_MIN: &str = "26.1.0";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    DockerConnection(#[from] ConnectionError),

    #[error("--image value {value:?} does not match the SEP-58 bldimg format `<registry-host>/<repo>@sha256:<64-hex>`. Examples: docker.io/stellar/stellar-cli@sha256:<64-hex>, localhost:5000/foo@sha256:<64-hex>. Tag-only refs and implicit Docker-Hub short refs are not accepted.")]
    BldimgFormat { value: String },

    #[error("could not determine the running rustc version: {0}")]
    RustcVersion(String),

    #[error("could not pull image {tag}: {detail}\n\nAvailable tags for this CLI version: {available_for_cli}\nAll published cli/rust pairs: {all_grouped}\n\nFix: install a matching rustc, or pass --image docker.io/stellar/stellar-cli@sha256:<digest> with one of the listed tags resolved to a digest.")]
    ImageNotFound {
        tag: String,
        available_for_cli: String,
        all_grouped: String,
        detail: String,
    },

    #[error("could not list published images on docker hub: {0}")]
    TagListUnavailable(String),

    #[error("image {tag} has no repo digest after pull; cannot record a content-addressed bldimg")]
    NoRepoDigest { tag: String },

    #[error("cargo metadata failed: {0}")]
    Metadata(#[from] cargo_metadata::Error),

    #[error(transparent)]
    SourceArchive(#[from] source_archive::Error),

    #[error(
        "the cli sets bldimg, source_uri, source_sha256, and bldopt automatically when --verifiable is used; remove them from --meta. Got reserved key: {key}"
    )]
    ReservedMetaKey { key: String },

    #[error("--source-sha256 value {value:?} does not match the SEP-58 source_sha256 format `^[0-9a-f]{{64}}$` (64-char lower-case hex).")]
    SourceSha256Format { value: String },

    #[error("--source-uri value {value:?} does not match the SEP-58 source_uri format `^[a-zA-Z][a-zA-Z0-9+.-]*:\\S+$` (a URI with a scheme, e.g. https://example.com/src.tar.gz).")]
    SourceUriFormat { value: String },

    #[error("--source-sha256 {provided} does not match the SHA-256 of the generated archive {computed}. Omit --source-sha256 to record the computed value, or fix the value.")]
    SourceSha256Mismatch { provided: String, computed: String },

    #[error(transparent)]
    Data(#[from] data::Error),

    #[error("container build exited with status {status}. To reproduce manually:\n  {command}")]
    ContainerExit { status: i64, command: String },

    #[error("verifiable build interrupted; stopped the build container")]
    Interrupted,
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

    // The source root is the current working directory: it's bind-mounted into
    // the container and the `--manifest-path` bldopt is relativized against it.
    // Run from the project/workspace root you want built. We do NOT validate that
    // it matches source_uri — a wrong source produces different bytes, and verify
    // catches that at byte-comparison time.
    let source_root = source_archive::resolve_source_root();

    // The archive is the working tree, so refuse a dirty repo: a verifiable build
    // should be deliberate, off a committed state, not whatever happens to be on
    // disk. Skipped when the source root isn't a git repo (we can't check, e.g.
    // archive sources).
    source_archive::ensure_clean_tree(&source_root, print).map_err(Error::from)?;

    // Always build the source archive, record its hash, and build from the
    // *extracted* archive (in a hardened tempdir) so the WASM is produced from
    // exactly the bytes that were hashed. A `--source-sha256` passed by the user
    // is treated as a pin and validated against the computed hash.
    let resolved = {
        let a = resolve_archive(cmd, &source_root, print)?;
        // The extracted `source/` dir mirrors `source_root` exactly and is both
        // the container mount and the tree the build writes `target/` into, so
        // it's what `collect_built_contracts` resolves artifacts against.
        let mount_root = a.extracted_root.join("source");
        ResolvedSource {
            source_sha256: a.source_sha256,
            extracted_root: Some(mount_root.clone()),
            mount_root,
            _tmp: Some(a.tmp),
        }
    };

    let source_ids = SourceIds {
        source_uri: cmd.source_uri.clone(),
        source_sha256: Some(resolved.source_sha256.clone()),
    };

    // Stage 3: the container engine. Every interaction shells out through these
    // `container_args`, which select the engine binary (`--engine`/
    // `STELLAR_CONTAINER_ENGINE`, default docker) and honor `--docker-host` where
    // the engine supports it; `run_args` carries the build container's resource
    // limits.
    let docker = cmd.container_args.clone();
    docker.warn_if_host_ignored(print);
    let image_ref = resolve_image(cmd, &docker, print).await?;

    // Only probe the container's cli version when we need to pick between
    // `--optimize=false` (new syntax) and not-forwarded-at-all (old default).
    // Bare `--optimize` is universally accepted, so the true path skips this.
    let supports_explicit_optimize_false = if cmd.build_args.optimize {
        true
    } else {
        probe_supports_optimize_false_syntax(&image_ref, &docker, print).await
    };

    // `--locked` is implied by `--verifiable`, but it was only added to
    // `contract build` in cli 25.2.0. Probe the image before adding it so a
    // build against an older, still-valid bldimg doesn't fail on an unknown
    // flag. When the flag is unavailable we drop it and warn that the rebuild
    // can't be pinned against dependency drift.
    let supports_locked = probe_supports_locked(&image_ref, &docker, print).await;
    if supports_locked {
        if !cmd.locked {
            print.infoln("Implying --locked because --verifiable was passed");
        }
    } else {
        print.warnln(
            "The build image's `contract build` does not support --locked; \
             building without it. Dependency drift may affect reproducibility.",
        );
    }

    // Build once per package, each with its own `--package` forwarded and
    // recorded as a `bldopt`, so every WASM is independently reproducible. With
    // no explicit `--package` the targets are inferred like a regular build.
    let packages = resolve_build_packages(cmd)?;
    if cmd.package.is_none() && !packages.is_empty() {
        print.infoln(format!("Inferred packages: {}", packages.join(", ")));
    }
    let targets: Vec<Option<&str>> = if packages.is_empty() {
        vec![None]
    } else {
        packages.iter().map(|p| Some(p.as_str())).collect()
    };
    let container_cmds: Vec<Vec<String>> = targets
        .iter()
        .map(|target| {
            let (forwarded_args, bldopts) = build_forwarded_args(
                cmd,
                &source_root,
                *target,
                supports_explicit_optimize_false,
                supports_locked,
            );
            let metadata_args = build_metadata_args(&image_ref, &source_ids, &bldopts);
            compose_container_args(&forwarded_args, &metadata_args)
        })
        .collect();

    // Always stream the container's cargo output during `contract build
    // --verifiable`, matching how a non-verifiable `contract build` shows
    // cargo output by default. The verify-side caller gates this on
    // `--verbose` because verifications are run as part of pipelines. All
    // per-package builds run in one container so the crates download, compiled
    // deps, and target/ are shared.
    let env: Vec<String> = cmd
        .build_args
        .env
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();

    run_in_container(
        &image_ref,
        &resolved.mount_root,
        &container_cmds,
        &env,
        &docker,
        &cmd.run_args,
        print,
        true,
    )
    .await?;

    let _ = global_args;
    let _ = workspace_root;
    collect_built_contracts(cmd, &source_root, resolved.extracted_root.as_deref(), print)
}

/// The recorded `source_sha256`, the directory bind-mounted at `/source`, the
/// extracted-archive root, and its tempdir guard — held so the temp dir
/// outlives the container build and artifact collection.
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

/// Source-identification fields recorded as SEP-58 meta. `source_sha256` is
/// always `Some` by the time these are built in `run()` (resolved from
/// `--source-sha256` or computed from the generated archive). `source_uri` is
/// `Some` only when the user passed `--source-uri`.
#[derive(Debug, Default, Clone)]
pub(crate) struct SourceIds {
    pub(crate) source_uri: Option<String>,
    pub(crate) source_sha256: Option<String>,
}

/// Format-validate the user-supplied source flags. Both are optional under
/// `--verifiable`; `--source-sha256`, when present, is validated as a pin in
/// `resolve_archive`.
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

/// Outcome of archiving: the generated archive's SHA-256 and the directory it
/// was extracted into (held alive by `tmp`).
struct ArchiveResult {
    source_sha256: String,
    extracted_root: PathBuf,
    tmp: tempfile::TempDir,
}

/// Build the source archive, record its hash, write it to the managed archives
/// dir (content-addressed, so the bytes are available to upload for
/// `--source-uri`), and extract it into a permission-hardened tempdir that the
/// container then builds from.
fn resolve_archive(cmd: &Cmd, source_root: &Path, print: &Print) -> Result<ArchiveResult, Error> {
    let bytes = source_archive::build_source_archive(source_root, print, true)?;
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

    // Content-addressed name under the managed archives dir.
    let out_path = data::archives_dir()?.join(format!("{computed}.tar.gz"));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| source_archive::Error::ArchiveWrite {
            path: out_path.clone(),
            source,
        })?;
    }
    std::fs::write(&out_path, &bytes).map_err(|source| source_archive::Error::ArchiveWrite {
        path: out_path.clone(),
        source,
    })?;
    print.infoln(format!(
        "Wrote source archive {} (source_sha256 {computed})",
        out_path.display()
    ));

    // Extract and harden, then build from the extracted copy so the WASM is
    // produced from exactly the archived bytes.
    let tmp = source_archive::extract_into_hardened_tempdir(&bytes, "verifiable-src-")?;
    let extracted_root = tmp.path().to_path_buf();
    Ok(ArchiveResult {
        source_sha256: computed,
        extracted_root,
        tmp,
    })
}

pub(crate) fn bldimg_regex() -> Regex {
    Regex::new(r"^(?:localhost(?::\d+)?|[^\s@/]*[.:][^\s@/]*)/[^\s@]+@sha256:[0-9a-f]{64}$")
        .unwrap()
}

pub(crate) fn source_sha256_regex() -> Regex {
    Regex::new(r"^[0-9a-f]{64}$").unwrap()
}

pub(crate) fn source_uri_regex() -> Regex {
    Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*:\S+$").unwrap()
}

/// Resolve every package the build will produce, so each can be pinned with its
/// own `--package` (and recorded as a `bldopt`) — making each WASM independently
/// reproducible even if the workspace's default members change later. An
/// explicit `--package` wins; otherwise infer the default-member cdylibs exactly
/// like a regular `stellar contract build` does. May be empty (no cdylib default
/// members), in which case the caller falls back to a single no-`--package`
/// build.
fn resolve_build_packages(cmd: &Cmd) -> Result<Vec<String>, Error> {
    if let Some(pkg) = &cmd.package {
        return Ok(vec![pkg.clone()]);
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
    Ok(names)
}

/// The flags forwarded to the container's `stellar contract build`, plus the
/// bldopt strings recorded into SEP-58 metadata. Every build-affecting flag
/// becomes one bldopt entry so a verifier can replay the same invocation.
/// `manifest_path` (when set) is recorded relative to the workspace root so it's
/// valid inside `/source`.
///
/// `supports_locked`: whether the container's `contract build` accepts
/// `--locked` (added in cli 25.2.0). When false the flag is neither forwarded
/// nor recorded, so a build against an older image doesn't fail on an unknown
/// argument.
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
    supports_locked: bool,
) -> (Vec<String>, Vec<String>) {
    let mut forwarded: Vec<String> = Vec::new();
    let mut bldopts: Vec<String> = Vec::new();

    // Record a build option. `None` means a bare flag (`--locked`); `Some(v)`
    // means `--flag=v`. The forwarded copy keeps the value raw (the container
    // gets it as argv, and `compose_shell_command` re-escapes it for the
    // multi-package `sh -c`); the bldopt copy shell-escapes the value once, here
    // at the source, so every recorded option is valid shell on its own and no
    // consumer has to split a flag from its value later. For `key=value`
    // payloads (`--meta`, `--env`) the key goes in `key` (`--meta=home_domain`)
    // and only the value is escaped, keeping `--env=B='nice value'` rather than
    // `'--env=B=nice value'`.
    let mut record = |key: &str, value: Option<&str>| {
        if let Some(v) = value {
            forwarded.push(format!("{key}={v}"));
            bldopts.push(format!("{key}={}", shell_escape::escape(v.into())));
        } else {
            forwarded.push(key.to_string());
            bldopts.push(key.to_string());
        }
    };

    if supports_locked {
        record("--locked", None);
    }

    if let Some(path) = &cmd.manifest_path {
        let abs = std::path::absolute(path).unwrap_or_else(|_| path.clone());
        let rel = abs
            .strip_prefix(workspace_root)
            .map(Path::to_path_buf)
            .unwrap_or(abs);
        record("--manifest-path", Some(rel.display().to_string().as_str()));
    }
    if cmd.profile != "release" {
        record("--profile", Some(cmd.profile.as_str()));
    }
    if let Some(features) = &cmd.features {
        record("--features", Some(features.as_str()));
    }
    if cmd.all_features {
        record("--all-features", None);
    }
    if cmd.no_default_features {
        record("--no-default-features", None);
    }
    // Always pin the package when it can be resolved (explicit `--package`, or
    // a workspace that builds exactly one cdylib by default) so the recorded
    // bldopt stays reproducible even if workspace default members change later.
    if let Some(pkg) = package {
        record("--package", Some(pkg));
    }
    for (k, v) in &cmd.build_args.meta {
        record(&format!("--meta={k}"), Some(v.as_str()));
    }

    // `--optimize` true is recorded as a bare flag (universally accepted).
    // `--optimize=false` is only emitted when the container's cli accepts it
    // (added in `b17d3f0b`); on older containers, false is the default and
    // we record/forward nothing — passing `--optimize=false` there would fail.
    if cmd.build_args.optimize {
        record("--optimize", None);
    } else if supports_explicit_optimize_false {
        record("--optimize", Some("false"));
    }

    // Build env vars are applied via docker `-e` (see run_in_container), not as
    // arguments to the inner `stellar contract build`, so they're recorded as
    // bldopts only — never forwarded. A verifier replays them with `--env`. The
    // value is escaped (the name is a validated identifier) so the recorded
    // option stays valid shell.
    for (name, value) in &cmd.build_args.env {
        bldopts.push(format!(
            "--env={name}={}",
            shell_escape::escape(value.as_str().into())
        ));
    }

    (forwarded, bldopts)
}

pub(crate) fn build_metadata_args(
    image_ref: &str,
    ids: &SourceIds,
    bldopts: &[String],
) -> Vec<String> {
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

    // bldopts already arrive as valid shell (escaped at the source in
    // `build_forwarded_args`), so they're recorded verbatim: a verifier
    // reconstructs the build by joining the recorded values and running them
    // through a shell.
    for o in bldopts {
        push(&mut out, "bldopt", o);
    }

    out
}

pub(crate) fn compose_container_args(forwarded: &[String], metadata: &[String]) -> Vec<String> {
    let mut args = vec!["contract".to_string(), "build".to_string()];
    args.extend_from_slice(forwarded);
    args.extend_from_slice(metadata);
    args
}

pub async fn resolve_image(
    cmd: &Cmd,
    docker: &shared::Args,
    print: &Print,
) -> Result<String, Error> {
    if let Some(s) = &cmd.image {
        if !bldimg_regex().is_match(s) {
            return Err(Error::BldimgFormat { value: s.clone() });
        }
        // Always pull, even when the digest is user-supplied. Docker requires
        // the image to be locally present before `docker run` will accept it,
        // and the user typically expects the cli to fetch whatever they asked
        // for.
        docker.pull_image(s, print).await?;
        return Ok(s.clone());
    }

    let cli_v = env!("CARGO_PKG_VERSION");
    let rust_v = rustc_version::version()
        .map_err(|e| Error::RustcVersion(e.to_string()))?
        .to_string();
    let tag = format!("{REGISTRY}:{cli_v}-rust{rust_v}");

    print.infoln(format!("Pulling verifiable build image {tag}"));

    match docker.pull_image(&tag, print).await {
        Ok(()) => {}
        // A failed pull of the derived cli/rust tag usually means no image was
        // published for this pair; turn it into the tag-listing hint. A missing
        // `docker` binary (or other connection failure) propagates as-is.
        Err(ConnectionError::PullImageFailed { stderr, .. }) => {
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
                detail: stderr,
            });
        }
        Err(e) => return Err(Error::DockerConnection(e)),
    }

    // Pin the mutable tag to the content-addressed digest the engine resolved,
    // so the recorded `bldimg` names the exact bytes that were pulled.
    docker
        .image_repo_digest(&tag)
        .await?
        .ok_or(Error::NoRepoDigest { tag })
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
    docker: &shared::Args,
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

/// Run `cmd` in a throwaway `docker run --rm` container (optionally overriding
/// the entrypoint) and return its captured stdout. Only stdout is collected;
/// stderr and the exit status are ignored, matching how every probe treats a
/// missing subcommand or unexpected output as "unsupported". Shared by every
/// image probe (cli version, active toolchain, flag support).
async fn run_probe(
    image_ref: &str,
    docker: &shared::Args,
    entrypoint: Option<&str>,
    cmd: Vec<String>,
) -> Result<String, Error> {
    let mut command = docker.base_command();
    command.args(["run", "--rm"]);
    if let Some(entrypoint) = entrypoint {
        command.args(["--entrypoint", entrypoint]);
    }
    command.arg(image_ref);
    command.args(&cmd);

    let output = command.output().await.map_err(|e| docker.io_error(e))?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn probe_cli_version(image_ref: &str, docker: &shared::Args) -> Result<Version, Error> {
    let stdout = run_probe(
        image_ref,
        docker,
        None,
        vec!["version".to_string(), "--only-version".to_string()],
    )
    .await?;
    Version::parse(stdout.trim())
        .map_err(|e| Error::TagListUnavailable(format!("unparseable version {stdout:?}: {e}")))
}

/// Probe whether the container's `stellar contract build` accepts `--locked`.
/// The flag was added in cli 25.2.0 (commit `6115b818`); older images reject it
/// outright, which would fail the build. Rather than map versions, ask the
/// container's own `contract build --help` whether the flag exists. On any probe
/// failure returns false — the conservative assumption that the flag is absent,
/// so the build proceeds without it rather than erroring.
pub(crate) async fn probe_supports_locked(
    image_ref: &str,
    docker: &shared::Args,
    print: &Print,
) -> bool {
    match run_probe(
        image_ref,
        docker,
        None,
        vec![
            "contract".to_string(),
            "build".to_string(),
            "--help".to_string(),
        ],
    )
    .await
    {
        Ok(help) => help.contains("--locked"),
        Err(e) => {
            print.warnln(format!(
                "Could not probe whether the container's `contract build` supports --locked ({e}); building without it"
            ));
            false
        }
    }
}

/// Probe the image for the toolchain rustup uses by default, so it can be
/// pinned via `RUSTUP_TOOLCHAIN` (see `run_in_container`). Overrides the
/// entrypoint to run `rustup show active-toolchain` and returns the toolchain
/// name — the first whitespace-delimited token, dropping any trailing
/// `(default)` marker (e.g. `1.93.0-x86_64-unknown-linux-gnu`). Returns `None`
/// on any failure (e.g. an image without rustup), so the build proceeds without
/// the pin rather than failing.
async fn probe_active_toolchain(image_ref: &str, docker: &shared::Args) -> Option<String> {
    let stdout = run_probe(
        image_ref,
        docker,
        Some("rustup"),
        vec!["show".to_string(), "active-toolchain".to_string()],
    )
    .await
    .ok()?;
    stdout.split_whitespace().next().map(str::to_string)
}

/// Render the per-package `stellar contract build …` commands into a single
/// `sh -c` script (`stellar … && stellar …`), shell-escaping every token so meta
/// values with spaces survive. Used when more than one package is built so they
/// share one container (and its crates download / compiled deps / `target/`).
fn compose_shell_command(cmds: &[Vec<String>]) -> String {
    cmds.iter()
        .map(|cmd| {
            std::iter::once("stellar")
                .chain(cmd.iter().map(String::as_str))
                .map(|tok| shell_escape::escape(tok.into()).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" && ")
}

/// Shell-escape each token of a single-package container command so a value
/// with spaces (a `--meta` value, or an `--env=` recorded as a `bldopt`)
/// survives when the reproduce line is copy-pasted into a shell. The
/// single-package path runs the image's default `stellar` entrypoint directly,
/// so there's no `sh -c` wrapper as in `compose_shell_command`.
fn escape_container_args(cmd: &[String]) -> String {
    cmd.iter()
        .map(|tok| shell_escape::escape(tok.into()).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resolve once the process receives any catchable signal that would otherwise
/// terminate it, so the caller can stop the build container before exiting.
/// `SIGKILL` can't be caught, so a `kill -9` still orphans the container —
/// nothing in-process can prevent that. On non-Unix platforms only Ctrl-C is
/// observable.
#[cfg(unix)]
async fn wait_for_termination_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    // If any handler fails to install we simply never resolve on that signal;
    // the build still runs, it just won't self-clean on that particular signal.
    let mut sigint = signal(SignalKind::interrupt());
    let mut sigterm = signal(SignalKind::terminate());
    let mut sighup = signal(SignalKind::hangup());
    let mut sigquit = signal(SignalKind::quit());

    tokio::select! {
        () = recv_signal(&mut sigint) => {},
        () = recv_signal(&mut sigterm) => {},
        () = recv_signal(&mut sighup) => {},
        () = recv_signal(&mut sigquit) => {},
    }
}

/// Await one delivery of an installed signal. When the handler failed to
/// install, never resolves, so it drops out of the `select!` above rather than
/// firing spuriously.
#[cfg(unix)]
async fn recv_signal(s: &mut std::io::Result<tokio::signal::unix::Signal>) {
    match s {
        Ok(s) => {
            s.recv().await;
        }
        Err(_) => std::future::pending().await,
    }
}

#[cfg(not(unix))]
async fn wait_for_termination_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_in_container(
    image_ref: &str,
    workspace_root: &Path,
    container_cmds: &[Vec<String>],
    env: &[String],
    docker: &shared::Args,
    run_args: &shared::RunArgs,
    print: &Print,
    verbose: bool,
) -> Result<(), Error> {
    let bind = format!("{}:/source", workspace_root.display());

    // Pin rustup to the image's own toolchain (per SEP-58): without this, a
    // `rust-toolchain.toml` in the source could make rustup switch toolchains
    // mid-build, defeating the digest-pinned image. Probe the image for its
    // active toolchain and pass it through with `-e`, unless the caller already
    // set RUSTUP_TOOLCHAIN. Skipped silently when the image has no rustup.
    let mut env = env.to_vec();
    if !env.iter().any(|e| e.starts_with("RUSTUP_TOOLCHAIN=")) {
        if let Some(toolchain) = probe_active_toolchain(image_ref, docker).await {
            env.push(format!("RUSTUP_TOOLCHAIN={toolchain}"));
        }
    }

    // `-e KEY=VALUE` flags for the reproduce command, mirroring the env passed
    // to the container below.
    let mut env_flags = String::new();
    for e in &env {
        env_flags.push_str(" -e ");
        env_flags.push_str(&shell_escape::escape(e.as_str().into()));
    }

    // Render the reproduce line against the engine binary the CLI actually ran
    // (`docker`, `container`), so a third party can replay the exact build.
    let program = docker.program();

    // Resource limits go right after `--rm`, matching where they're applied to
    // the spawned command below, so the reproduce line stays copy-paste faithful.
    let mut run_flags = String::new();
    for f in run_args.flags() {
        run_flags.push(' ');
        run_flags.push_str(&shell_escape::escape(f.into()));
    }

    // One package → run the image's default `stellar` entrypoint directly, so
    // `post_image` is just the `contract build …` argv. Several → override the
    // entrypoint to a shell and chain the builds so they all run in this one
    // container; `--entrypoint` takes only the executable, so the `-c <chain>`
    // arguments follow the image name in `post_image`.
    let (entrypoint, post_image, reproduce) = if container_cmds.len() > 1 {
        let chain = compose_shell_command(container_cmds);
        let reproduce = format!(
            "{program} run --rm{run_flags} -v {bind}{env_flags} --entrypoint /bin/sh {image_ref} -c {}",
            shell_escape::escape(chain.clone().into())
        );
        (Some("/bin/sh"), vec!["-c".to_string(), chain], reproduce)
    } else {
        let cmd = container_cmds.first().cloned().unwrap_or_default();
        let reproduce = format!(
            "{program} run --rm{run_flags} -v {bind}{env_flags} {image_ref} {}",
            escape_container_args(&cmd)
        );
        (None, cmd, reproduce)
    };

    print.infoln(format!(
        "Running verifiable build in {image_ref} (mount {bind})"
    ));
    if verbose {
        print.infoln(format!("Running: {reproduce}"));
    }

    // Name the build container so it can be stopped if the CLI is interrupted.
    // Without a name there's no handle to target: on a termination signal the
    // CLI process dies, but the container the daemon owns keeps running (the
    // engine client exiting doesn't stop it, and signal-forwarding through the
    // client is unreliable for a long cargo build). `--rm` removes it once
    // stopped. The name is unique per invocation (pid + random) so concurrent
    // builds don't collide, and it's kept out of the reproduce line — a fixed
    // name there would clash on re-run.
    let container_name = format!(
        "stellar-verifiable-build-{}-{:08x}",
        std::process::id(),
        rand::random::<u32>()
    );

    let mut command = docker.base_command();
    command.args(["run", "--rm", "--name", &container_name]);
    run_args.apply(&mut command);
    command.args(["-v", &bind, "-w", "/source"]);
    for e in &env {
        command.args(["-e", e]);
    }
    if let Some(entrypoint) = entrypoint {
        command.args(["--entrypoint", entrypoint]);
    }
    command.arg(image_ref);
    command.args(&post_image);

    // Stream the build's cargo output straight to the terminal when verbose
    // (matching a non-verifiable `contract build`); otherwise discard it (the
    // verify pipeline suppresses per-build noise). `quiet` overrides verbose.
    let show_output = verbose && !print.quiet;
    let (stdout, stderr) = if show_output {
        (Stdio::inherit(), Stdio::inherit())
    } else {
        (Stdio::null(), Stdio::null())
    };
    command.stdout(stdout).stderr(stderr);

    let mut child = command.spawn().map_err(|e| docker.io_error(e))?;

    // Race the build against any catchable termination signal. On a signal,
    // stop the named container (best-effort) so it doesn't outlive the CLI,
    // kill the engine client we spawned, then surface the interruption.
    let status = tokio::select! {
        result = child.wait() => result.map_err(|e| docker.io_error(e))?,
        () = wait_for_termination_signal() => {
            print.warnln("Interrupted; stopping build container");
            // `kill` (SIGKILL, immediate) rather than `stop` (SIGTERM + 10s
            // grace): otherwise the container keeps building for the whole grace
            // period while we block here.
            let _ = docker.kill_command(&container_name).output().await;
            let _ = child.start_kill();
            return Err(Error::Interrupted);
        }
    };
    if !status.success() {
        return Err(Error::ContainerExit {
            status: status.code().unwrap_or(-1).into(),
            command: reproduce,
        });
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
        let (forwarded, bldopts) =
            build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true, true);
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
    fn build_forwarded_args_omits_locked_when_unsupported() {
        // Older images (< cli 25.2.0) reject `--locked`; when the probe reports
        // it's unsupported, the flag is neither forwarded nor recorded.
        let cmd = Cmd::default();
        let (forwarded, bldopts) =
            build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true, false);
        assert!(!forwarded.iter().any(|a| a == "--locked"));
        assert!(!bldopts.iter().any(|a| a == "--locked"));
        // Everything else is still recorded (default optimize=true here).
        assert!(forwarded.contains(&"--optimize".to_string()));
    }

    #[test]
    fn build_forwarded_args_features_and_package() {
        let cmd = Cmd {
            features: Some("a,b".to_string()),
            package: Some("contract-a".to_string()),
            ..Cmd::default()
        };
        let (forwarded, bldopts) =
            build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true, true);
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
        let (forwarded, bldopts) =
            build_forwarded_args(&cmd, ws(), Some("hello-world"), true, true);
        assert!(forwarded.contains(&"--package=hello-world".to_string()));
        assert!(bldopts.contains(&"--package=hello-world".to_string()));
    }

    #[test]
    fn build_forwarded_args_omits_package_when_unresolved() {
        let cmd = Cmd::default();
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), None, true, true);
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
                env: vec![],
                optimize: true,
            },
            ..Cmd::default()
        };
        let (forwarded, bldopts) =
            build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true, true);
        assert!(forwarded.contains(&"--meta=home_domain=fnando.com".to_string()));
        assert!(forwarded.contains(&"--meta=author=alice".to_string()));
        assert!(forwarded.contains(&"--manifest-path=contracts/add/Cargo.toml".to_string()));
        assert!(bldopts.contains(&"--meta=home_domain=fnando.com".to_string()));
        assert!(bldopts.contains(&"--meta=author=alice".to_string()));
        assert!(bldopts.contains(&"--manifest-path=contracts/add/Cargo.toml".to_string()));
    }

    #[test]
    fn build_forwarded_args_records_env_as_bldopt_only() {
        let cmd = Cmd {
            build_args: super::super::BuildArgs {
                env: vec![
                    ("FOO".to_string(), "bar".to_string()),
                    ("BAZ".to_string(), "qux".to_string()),
                ],
                ..super::super::BuildArgs::default()
            },
            ..Cmd::default()
        };
        let (forwarded, bldopts) =
            build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true, true);
        // Env vars are applied via docker `-e`, so they're recorded as bldopts
        // for the verifier but never forwarded as build arguments.
        assert!(bldopts.contains(&"--env=FOO=bar".to_string()));
        assert!(bldopts.contains(&"--env=BAZ=qux".to_string()));
        assert!(!forwarded.iter().any(|a| a.starts_with("--env")));
    }

    #[test]
    fn build_forwarded_args_optimize_false_new_container() {
        let cmd = Cmd {
            build_args: super::super::BuildArgs {
                meta: vec![],
                env: vec![],
                optimize: false,
            },
            ..Cmd::default()
        };
        let (forwarded, bldopts) =
            build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true, true);
        assert!(forwarded.contains(&"--optimize=false".to_string()));
        assert!(bldopts.contains(&"--optimize=false".to_string()));
    }

    #[test]
    fn build_forwarded_args_optimize_false_old_container() {
        let cmd = Cmd {
            build_args: super::super::BuildArgs {
                meta: vec![],
                env: vec![],
                optimize: false,
            },
            ..Cmd::default()
        };
        let (forwarded, bldopts) =
            build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), false, true);
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
    fn build_forwarded_args_escapes_bldopt_values_as_shell() {
        // Values with shell metacharacters are escaped at the source so each
        // recorded bldopt is valid shell on its own. Only the value side is
        // quoted: `--env=B='this is very nice'`, never `'--env=B=this is very
        // nice'` (which would quote the flag and key too).
        let cmd = Cmd {
            features: Some("a,b".to_string()),
            build_args: super::super::BuildArgs {
                meta: vec![("note".to_string(), "added on build".to_string())],
                env: vec![
                    ("B".to_string(), "this is very nice".to_string()),
                    ("C".to_string(), "it's a \"trap\"".to_string()),
                ],
                optimize: true,
            },
            ..Cmd::default()
        };
        let (_forwarded, bldopts) =
            build_forwarded_args(&cmd, ws(), cmd.package.as_deref(), true, true);

        // The flag and key stay outside the quotes; only the value is escaped.
        assert!(bldopts.contains(&"--env=B='this is very nice'".to_string()));
        assert!(bldopts.contains(&"--meta=note='added on build'".to_string()));
        // No-metacharacter values stay verbatim.
        assert!(bldopts.contains(&"--features=a,b".to_string()));

        // Every recorded bldopt is valid shell that parses back to one argv token.
        for o in &bldopts {
            let tokens = shlex::split(o).expect("each bldopt must be valid shell");
            assert_eq!(tokens.len(), 1, "{o} must be a single shell token");
        }
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

    #[test]
    fn compose_shell_command_chains_and_escapes() {
        let a = vec![
            "contract".to_string(),
            "build".to_string(),
            "--package=another".to_string(),
            "--meta".to_string(),
            "home_domain=fnando.com".to_string(),
        ];
        let b = vec![
            "contract".to_string(),
            "build".to_string(),
            "--package=hello-world".to_string(),
        ];
        let s = compose_shell_command(&[a, b]);
        assert_eq!(
            s,
            "stellar contract build --package=another --meta home_domain=fnando.com \
             && stellar contract build --package=hello-world"
        );

        // A meta value with a space must be quoted so it stays one token.
        let c = vec![
            "contract".to_string(),
            "build".to_string(),
            "--meta".to_string(),
            "note=added on build".to_string(),
        ];
        let s = compose_shell_command(&[c]);
        assert!(
            s.contains("'note=added on build'") || s.contains("\"note=added on build\""),
            "expected the spaced value to be quoted, got: {s}"
        );
    }

    #[test]
    fn escape_container_args_quotes_spaced_tokens() {
        // An `--env=` recorded as a bldopt carries the env value verbatim, so a
        // spaced value lands in a single `--meta bldopt=…` token. The reproduce
        // line must quote it so a copy-paste round-trips back to one argv token.
        let cmd = vec![
            "contract".to_string(),
            "build".to_string(),
            "--package=hello-world".to_string(),
            "--meta".to_string(),
            "bldopt=--env=B=this is very nice".to_string(),
        ];
        let s = escape_container_args(&cmd);
        let tokens = shlex::split(&s).expect("reproduce args must be valid shell");
        assert_eq!(
            tokens,
            vec![
                "contract",
                "build",
                "--package=hello-world",
                "--meta",
                "bldopt=--env=B=this is very nice",
            ],
            "spaced token must survive a shlex round-trip as one argument"
        );
    }
}
