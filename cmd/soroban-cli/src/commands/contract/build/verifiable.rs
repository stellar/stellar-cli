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

    #[error("--image must be digest-pinned (got {value}); SEP-58 requires content-addressed images. Pass docker.io/stellar/stellar-cli@sha256:<digest>")]
    ImageNotDigestPinned { value: String },

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
        "git working tree at {path} is dirty. Verifiable builds require a clean tree so the recorded source_rev matches the WASM bytes. Commit or stash your changes and try again."
    )]
    GitDirty { path: PathBuf },

    #[error(
        "the cli sets bldimg, source_rev, and bldopt automatically when --verifiable is used; remove them from --meta. Got reserved key: {key}"
    )]
    ReservedMetaKey { key: String },

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
        if !img.contains("@sha256:") {
            return Err(Error::ImageNotDigestPinned { value: img.clone() }.into());
        }
    }

    if !cmd.locked {
        print.infoln("--verifiable implies --locked");
    }

    // Stage 2: local filesystem + git, no network.
    let workspace_root = resolve_workspace_root(cmd)?;
    let source_rev = git_source_rev(&workspace_root, print)?;

    // Stage 3: docker.
    let docker = cmd
        .container_args
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
    let metadata_args = build_metadata_args(&image_ref, &source_rev, &bldopts);
    let container_cmd_args = compose_container_args(&forwarded_args, &metadata_args);

    run_in_container(
        &image_ref,
        &workspace_root,
        &container_cmd_args,
        &docker,
        print,
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

fn git_source_rev(workspace_root: &Path, print: &Print) -> Result<String, Error> {
    // Probe with rev-parse first to detect "not a git repo".
    let rev = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .arg("rev-parse")
        .arg("HEAD")
        .output();
    let rev = match rev {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Ok(_) => {
            print.warnln(format!(
                "{} is not a git repository; recording empty source_rev (verifiability is degraded).",
                workspace_root.display()
            ));
            return Ok(String::new());
        }
        Err(e) => {
            return Err(Error::GitInvoke {
                path: workspace_root.to_path_buf(),
                source: e,
            })
        }
    };

    // Dirty check.
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

    Ok(rev)
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

fn build_metadata_args(image_ref: &str, source_rev: &str, bldopts: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in [("bldimg", image_ref), ("source_rev", source_rev)] {
        out.push("--meta".to_string());
        out.push(format!("{k}={v}"));
    }
    for o in bldopts {
        out.push("--meta".to_string());
        out.push(format!("bldopt={o}"));
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
        if !s.contains("@sha256:") {
            return Err(Error::ImageNotDigestPinned { value: s.clone() });
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
                "could not probe container cli version ({e}); assuming pre-{OPTIMIZE_NEW_SYNTAX_MIN} syntax"
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
                let s = String::from_utf8_lossy(&message);
                print.blankln(s.trim_end());
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

    #[test]
    fn build_metadata_args_orders_keys() {
        let m = build_metadata_args(
            "docker.io/stellar/stellar-cli@sha256:abc",
            "deadbeef",
            &["--locked".to_string(), "--features=a".to_string()],
        );
        // bldimg, source_rev, then bldopts in order.
        let pairs: Vec<(&str, &str)> = m
            .chunks(2)
            .map(|c| (c[0].as_str(), c[1].as_str()))
            .collect();
        assert_eq!(
            pairs[0],
            ("--meta", "bldimg=docker.io/stellar/stellar-cli@sha256:abc")
        );
        assert_eq!(pairs[1], ("--meta", "source_rev=deadbeef"));
        assert_eq!(pairs[2], ("--meta", "bldopt=--locked"));
        assert_eq!(pairs[3], ("--meta", "bldopt=--features=a"));
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
