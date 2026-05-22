use std::{
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

use crate::{
    commands::{container::shared::Error as ConnectionError, global},
    print::Print,
};

use super::{BuiltContract, Cmd, WASM_TARGET};

const REGISTRY: &str = "docker.io/stellar/stellar-cli";
const HUB_TAGS_URL: &str =
    "https://hub.docker.com/v2/repositories/stellar/stellar-cli/tags/?page_size=100";
const RESERVED_META_KEYS: &[&str] = &["bldimg", "source_rev", "bldopt"];

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
        "git working tree at {path} is dirty. --source-rev requires a clean tree so the recorded source_rev matches the WASM bytes. Commit or stash your changes and try again."
    )]
    GitDirty { path: PathBuf },

    #[error(
        "the cli sets bldimg, source_rev, and bldopt automatically when --verifiable is used; remove them from --meta. Got reserved key: {key}"
    )]
    ReservedMetaKey { key: String },

    #[error("--verifiable requires a SEP-58 source-identification combination. Pass one of: (--source-repo + --source-rev), (--tarball-url and/or --tarball-sha256).")]
    MissingSourceId,

    #[error("--source-rev value {value:?} does not match the SEP-58 source_rev format `^[0-9a-f]{{40}}$` (full 40-char SHA-1 of the source commit).")]
    SourceRevFormat { value: String },

    #[error("--source-repo value {value:?} does not match the SEP-58 source_repo format `^(https?://\\S+|github:[^/\\s]+/[^/\\s]+)$`.")]
    SourceRepoFormat { value: String },

    #[error("--tarball-url value {value:?} does not match the SEP-58 tarball_url format `^https?://\\S+$`.")]
    TarballUrlFormat { value: String },

    #[error("--tarball-sha256 value {value:?} does not match the SEP-58 tarball_sha256 format `^[0-9a-f]{{64}}$`.")]
    TarballSha256Format { value: String },

    #[error("--source-rev requires a git workspace at {path}; `git rev-parse HEAD` failed there.")]
    SourceRevNotGitRepo { path: PathBuf },

    #[error("--source-rev {claimed} does not match local HEAD {head}. Commit, switch, or pass the correct rev.")]
    SourceRevHeadMismatch { claimed: String, head: String },

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

    // Stage 2: local filesystem + git, no network. Resolve the workspace root
    // first so the (optional) `--source-rev` git cross-check has a path to
    // anchor on.
    let workspace_root = resolve_workspace_root(cmd)?;
    let source_ids = validate_source_ids(cmd, &workspace_root)?;

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

    let (forwarded_args, bldopts) =
        build_forwarded_args(cmd, &workspace_root, supports_explicit_optimize_false);
    let metadata_args = build_metadata_args(&image_ref, &source_ids, &bldopts);
    let container_cmd_args = compose_container_args(&forwarded_args, &metadata_args);

    // Always stream the container's cargo output during `contract build
    // --verifiable`, matching how a non-verifiable `contract build` shows
    // cargo output by default. The verify-side caller gates this on
    // `--verbose` because verifications are run as part of pipelines.
    run_in_container(
        &image_ref,
        &workspace_root,
        &container_cmd_args,
        &docker,
        print,
        true,
    )
    .await?;

    let _ = global_args;
    collect_built_contracts(cmd, &workspace_root, print)
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

/// Source-identification fields, gathered from the corresponding CLI flags
/// after validation. Each is `Some` only when the user passed the flag and the
/// value matched the SEP-58 format regex. The four fields cannot all be
/// `None` — `validate_source_ids` rejects that case.
#[derive(Debug, Default, Clone)]
struct SourceIds {
    source_repo: Option<String>,
    source_rev: Option<String>,
    tarball_url: Option<String>,
    tarball_sha256: Option<String>,
}

fn validate_source_ids(cmd: &Cmd, workspace_root: &Path) -> Result<SourceIds, Error> {
    let ids = SourceIds {
        source_repo: cmd.source_repo.clone(),
        source_rev: cmd.source_rev.clone(),
        tarball_url: cmd.tarball_url.clone(),
        tarball_sha256: cmd.tarball_sha256.clone(),
    };

    if ids.source_repo.is_none()
        && ids.source_rev.is_none()
        && ids.tarball_url.is_none()
        && ids.tarball_sha256.is_none()
    {
        return Err(Error::MissingSourceId);
    }

    if let Some(v) = &ids.source_rev {
        if !source_rev_regex().is_match(v) {
            return Err(Error::SourceRevFormat { value: v.clone() });
        }
    }

    if let Some(v) = &ids.source_repo {
        if !source_repo_regex().is_match(v) {
            return Err(Error::SourceRepoFormat { value: v.clone() });
        }
    }

    if let Some(v) = &ids.tarball_url {
        if !tarball_url_regex().is_match(v) {
            return Err(Error::TarballUrlFormat { value: v.clone() });
        }
    }

    if let Some(v) = &ids.tarball_sha256 {
        if !tarball_sha256_regex().is_match(v) {
            return Err(Error::TarballSha256Format { value: v.clone() });
        }
    }

    if let Some(claimed) = &ids.source_rev {
        cross_check_source_rev_against_git(workspace_root, claimed)?;
    }

    Ok(ids)
}

fn cross_check_source_rev_against_git(workspace_root: &Path, claimed: &str) -> Result<(), Error> {
    let rev_out = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|e| Error::GitInvoke {
            path: workspace_root.to_path_buf(),
            source: e,
        })?;

    if !rev_out.status.success() {
        return Err(Error::SourceRevNotGitRepo {
            path: workspace_root.to_path_buf(),
        });
    }

    let head = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();
    if head != claimed {
        return Err(Error::SourceRevHeadMismatch {
            claimed: claimed.to_string(),
            head,
        });
    }

    let status = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .map_err(|e| Error::GitInvoke {
            path: workspace_root.to_path_buf(),
            source: e,
        })?;

    if !status.stdout.is_empty() {
        return Err(Error::GitDirty {
            path: workspace_root.to_path_buf(),
        });
    }

    Ok(())
}

fn bldimg_regex() -> Regex {
    Regex::new(r"^(?:localhost(?::\d+)?|[^\s@/]*[.:][^\s@/]*)/[^\s@]+@sha256:[0-9a-f]{64}$")
        .unwrap()
}

fn source_rev_regex() -> Regex {
    Regex::new(r"^[0-9a-f]{40}$").unwrap()
}

fn source_repo_regex() -> Regex {
    Regex::new(r"^(https?://\S+|github:[^/\s]+/[^/\s]+)$").unwrap()
}

fn tarball_url_regex() -> Regex {
    Regex::new(r"^https?://\S+$").unwrap()
}

fn tarball_sha256_regex() -> Regex {
    Regex::new(r"^[0-9a-f]{64}$").unwrap()
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
    if let Some(pkg) = &cmd.package {
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

    if let Some(v) = &ids.source_repo {
        push(&mut out, "source_repo", v);
    }
    if let Some(v) = &ids.source_rev {
        push(&mut out, "source_rev", v);
    }
    if let Some(v) = &ids.tarball_url {
        push(&mut out, "tarball_url", v);
    }
    if let Some(v) = &ids.tarball_sha256 {
        push(&mut out, "tarball_sha256", v);
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
            if status.contains("Pulling from")
                || status.contains("Digest")
                || status.contains("Status")
            {
                print.infoln(status);
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

fn collect_built_contracts(
    cmd: &Cmd,
    workspace_root: &Path,
    _print: &Print,
) -> Result<Vec<BuiltContract>, super::Error> {
    let mut mc = MetadataCommand::new();
    mc.no_deps();
    if let Some(p) = &cmd.manifest_path {
        mc.manifest_path(p);
    }
    let md = mc.exec().map_err(Error::Metadata)?;
    let target_dir = md.target_directory.as_std_path();

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
        let path = Path::new(target_dir)
            .join(WASM_TARGET)
            .join(&cmd.profile)
            .join(format!("{wasm_name}.wasm"));
        if let Some(out_dir) = &cmd.out_dir {
            let dest = out_dir.join(format!("{wasm_name}.wasm"));
            if path.exists() {
                std::fs::create_dir_all(out_dir).map_err(super::Error::CreatingOutDir)?;
                std::fs::copy(&path, &dest).map_err(super::Error::CopyingWasmFile)?;
                out.push(BuiltContract {
                    name: p.name.clone(),
                    path: dest,
                });
                continue;
            }
        }
        out.push(BuiltContract {
            name: p.name.clone(),
            path,
        });
    }
    let _ = workspace_root;
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
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), true);
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
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), true);
        assert!(forwarded.contains(&"--features=a,b".to_string()));
        assert!(forwarded.contains(&"--package=contract-a".to_string()));
        assert!(bldopts.contains(&"--features=a,b".to_string()));
        assert!(bldopts.contains(&"--package=contract-a".to_string()));
        assert!(bldopts.contains(&"--locked".to_string()));
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
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), true);
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
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), true);
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
        let (forwarded, bldopts) = build_forwarded_args(&cmd, ws(), false);
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
    fn build_metadata_args_source_repo_and_rev() {
        let ids = SourceIds {
            source_repo: Some("https://github.com/foo/bar".to_string()),
            source_rev: Some("a".repeat(40)),
            tarball_url: None,
            tarball_sha256: None,
        };
        let m = build_metadata_args(
            "docker.io/stellar/stellar-cli@sha256:abc",
            &ids,
            &["--locked".to_string(), "--features=a".to_string()],
        );
        let p = pairs(&m);
        // bldimg first; source-ids only for what's set; bldopts last.
        assert_eq!(
            p[0],
            ("--meta", "bldimg=docker.io/stellar/stellar-cli@sha256:abc")
        );
        assert_eq!(p[1], ("--meta", "source_repo=https://github.com/foo/bar"));
        assert_eq!(p[2].0, "--meta");
        assert!(p[2].1.starts_with("source_rev="));
        assert_eq!(p[3], ("--meta", "bldopt=--locked"));
        assert_eq!(p[4], ("--meta", "bldopt=--features=a"));
        // No tarball entries emitted when those fields are None.
        assert!(!m.iter().any(|s| s.starts_with("tarball_")));
    }

    #[test]
    fn build_metadata_args_tarball_url_only() {
        let ids = SourceIds {
            tarball_url: Some("https://example.com/foo.tar.gz".to_string()),
            ..SourceIds::default()
        };
        let m = build_metadata_args("docker.io/stellar/stellar-cli@sha256:abc", &ids, &[]);
        assert!(m
            .iter()
            .any(|s| s == "tarball_url=https://example.com/foo.tar.gz"));
        assert!(!m.iter().any(|s| s.starts_with("source_")));
        assert!(!m.iter().any(|s| s.starts_with("tarball_sha256=")));
    }

    #[test]
    fn build_metadata_args_tarball_pair() {
        let ids = SourceIds {
            tarball_url: Some("https://example.com/foo.tar.gz".to_string()),
            tarball_sha256: Some("f".repeat(64)),
            ..SourceIds::default()
        };
        let m = build_metadata_args("docker.io/stellar/stellar-cli@sha256:abc", &ids, &[]);
        assert!(m
            .iter()
            .any(|s| s == "tarball_url=https://example.com/foo.tar.gz"));
        assert!(m
            .iter()
            .any(|s| s == &format!("tarball_sha256={}", "f".repeat(64))));
    }

    #[test]
    fn validate_source_ids_missing_all_errors() {
        let cmd = Cmd::default();
        let err = validate_source_ids(&cmd, ws()).unwrap_err();
        assert!(matches!(err, Error::MissingSourceId));
    }

    #[test]
    fn validate_source_ids_rejects_bad_source_rev_format() {
        let cmd = Cmd {
            source_repo: Some("https://github.com/foo/bar".to_string()),
            source_rev: Some("not-a-sha".to_string()),
            ..Cmd::default()
        };
        let err = validate_source_ids(&cmd, ws()).unwrap_err();
        assert!(matches!(err, Error::SourceRevFormat { .. }));
    }

    #[test]
    fn validate_source_ids_rejects_bad_source_repo_format() {
        let cmd = Cmd {
            source_repo: Some("foo/bar".to_string()), // missing scheme
            source_rev: Some("a".repeat(40)),
            ..Cmd::default()
        };
        let err = validate_source_ids(&cmd, ws()).unwrap_err();
        assert!(matches!(err, Error::SourceRepoFormat { .. }));
    }

    #[test]
    fn validate_source_ids_rejects_bad_tarball_url() {
        let cmd = Cmd {
            tarball_url: Some("ftp://example.com/foo.tar.gz".to_string()),
            ..Cmd::default()
        };
        let err = validate_source_ids(&cmd, ws()).unwrap_err();
        assert!(matches!(err, Error::TarballUrlFormat { .. }));
    }

    #[test]
    fn validate_source_ids_rejects_short_tarball_sha256() {
        let cmd = Cmd {
            tarball_sha256: Some("abc".to_string()),
            ..Cmd::default()
        };
        let err = validate_source_ids(&cmd, ws()).unwrap_err();
        assert!(matches!(err, Error::TarballSha256Format { .. }));
    }

    #[test]
    fn validate_source_ids_accepts_tarball_url_alone() {
        let cmd = Cmd {
            tarball_url: Some("https://example.com/foo.tar.gz".to_string()),
            ..Cmd::default()
        };
        let ids = validate_source_ids(&cmd, ws()).unwrap();
        assert_eq!(
            ids.tarball_url.as_deref(),
            Some("https://example.com/foo.tar.gz")
        );
        assert!(ids.source_repo.is_none());
        assert!(ids.source_rev.is_none());
        assert!(ids.tarball_sha256.is_none());
    }

    #[test]
    fn validate_source_ids_accepts_tarball_sha256_alone() {
        let cmd = Cmd {
            tarball_sha256: Some("f".repeat(64)),
            ..Cmd::default()
        };
        let ids = validate_source_ids(&cmd, ws()).unwrap();
        assert_eq!(
            ids.tarball_sha256.as_deref(),
            Some("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
        );
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
    fn source_rev_regex_matches_40_hex() {
        assert!(source_rev_regex().is_match(&"a".repeat(40)));
        assert!(!source_rev_regex().is_match(&"a".repeat(39)));
        assert!(!source_rev_regex().is_match(&"A".repeat(40))); // upper-case rejected
    }

    #[test]
    fn source_repo_regex_accepts_https_and_github_shorthand() {
        assert!(source_repo_regex().is_match("https://github.com/foo/bar"));
        assert!(source_repo_regex().is_match("http://example.com/foo.git"));
        assert!(source_repo_regex().is_match("github:foo/bar"));
        assert!(!source_repo_regex().is_match("foo/bar"));
        assert!(!source_repo_regex().is_match("git@github.com:foo/bar.git"));
    }

    #[test]
    fn tarball_url_regex_accepts_http_only() {
        assert!(tarball_url_regex().is_match("https://example.com/foo.tar.gz"));
        assert!(tarball_url_regex().is_match("http://example.com/foo.tar.gz"));
        assert!(!tarball_url_regex().is_match("ftp://example.com/foo.tar.gz"));
    }

    #[test]
    fn tarball_sha256_regex_matches_64_hex() {
        assert!(tarball_sha256_regex().is_match(&"f".repeat(64)));
        assert!(!tarball_sha256_regex().is_match(&"f".repeat(63)));
        assert!(!tarball_sha256_regex().is_match(&"F".repeat(64)));
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
        for key in ["bldimg", "source_rev", "bldopt"] {
            assert!(RESERVED_META_KEYS.contains(&key));
        }
    }
}
