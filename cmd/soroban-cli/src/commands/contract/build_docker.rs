//! `--backend docker` build backend.
//!
//! Runs the entire `stellar contract build` pipeline inside a container
//! whose entrypoint is `stellar` (the official `stellar/stellar-cli`
//! image, or any user-supplied image with the same shape). The host
//! orchestrates only — pull, set up bind mounts, run, stream logs.
//!
//! The host CLI's version is irrelevant: whatever cli is in the image is
//! what builds the wasm and what records `cliver` / `rsver` / source meta
//! into the wasm. The host injects `bldimg` (the pulled image's resolved
//! digest) via the inner cli's `--meta` mechanism — no new flags.
//!
//! For `verify`, the recorded `bldimg` is pulled (so the same cli runs)
//! and `RUSTUP_TOOLCHAIN` is set from the wasm's `rsver` so the rust
//! toolchain matches whatever the original build used.
//!
//! User-supplied images must:
//! - Have `stellar` as their entrypoint
//! - Have `rustup` available with the `wasm32v1-none` target installed
//!   (preflight-checked before the build runs)

use std::path::Path;

use bollard::{
    models::ContainerCreateBody,
    query_parameters::{
        CreateContainerOptions, CreateImageOptions, LogsOptions, RemoveContainerOptions,
        StartContainerOptions, WaitContainerOptions,
    },
    service::HostConfig,
    Docker,
};
use futures_util::{StreamExt, TryStreamExt};

use crate::{
    commands::container::shared::{Args as ContainerArgs, Error as ContainerError},
    print::Print,
};

const PLATFORM: &str = "linux/amd64";
/// Where the workspace gets bind-mounted inside the container. Matches the
/// official `stellar/stellar-cli` image's `WORKDIR`. Cargo writes its
/// target directory under this path, so the host reads the wasm via the
/// same bind mount — no separate `/target` mount needed.
pub const SOURCE_DIR: &str = "/source";
const REGISTRY_DIR: &str = "/usr/local/cargo/registry";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot connect to docker daemon; is it running? ({0})")]
    RuntimeNotRunning(ContainerError),

    #[error("pulling docker image {image}: {source}")]
    ImagePull {
        image: String,
        source: bollard::errors::Error,
    },

    #[error("inspecting docker image {image}: {source}")]
    ImageInspect {
        image: String,
        source: bollard::errors::Error,
    },

    #[error("docker image {image} has no repository digest. Either pin via --backend docker=<registry>/<image>@sha256:..., or remove any locally-built image at this tag (`docker rmi {image}`) and let the default re-pull")]
    NoDigest { image: String },

    #[error("pulling docker image {image}: daemon reported error: {message}")]
    PullDaemonError { image: String, message: String },

    #[error("build failed inside docker container (exit {0})")]
    BuildExit(i64),

    #[error("docker run: {0}")]
    Runtime(#[from] bollard::errors::Error),

    #[error("resolving CARGO_HOME: {0}")]
    CargoHome(std::io::Error),
}

/// Forwarded host build args used to construct the inner
/// `stellar contract build` invocation. `manifest_path` is expected to
/// already be in container-relative form (`/source/...`). `meta` holds both
/// the user's `--meta` entries and any host-detected entries (e.g.
/// `source_repo`, `source_rev`, `bldopt_*`) that are forwarded as
/// transitional pass-throughs while the published cli image catches up.
pub struct InnerBuildArgs<'a> {
    pub manifest_path: String,
    pub package: Option<&'a str>,
    pub profile: &'a str,
    pub features: Option<&'a str>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub optimize: bool,
    pub meta: Vec<(String, String)>,
}

/// Pull the image (if needed), then run the in-container
/// `stellar contract build --backend local --meta bldimg=<digest>` against
/// the bind-mounted source. Returns the resolved image digest for the host
/// to record.
///
/// `rsver` is `None` for fresh builds and `Some(<wasm's rsver>)` for verify;
/// when set, `RUSTUP_TOOLCHAIN` inside the container is pinned to that
/// toolchain so rustup-managed cargo uses the matching rust version.
#[allow(clippy::too_many_arguments)]
pub async fn run_in_docker(
    image: &str,
    pre_resolved: Option<&str>,
    rsver: Option<&str>,
    mount_root: &Path,
    inner: &InnerBuildArgs<'_>,
    container_args: &ContainerArgs,
    print: &Print,
) -> Result<String, Error> {
    let docker: Docker = container_args
        .connect_to_docker(print)
        .await
        .map_err(Error::RuntimeNotRunning)?;

    let resolved = if let Some(r) = pre_resolved {
        r.to_string()
    } else {
        // Skip the pull when the image is already local. For digest-pinned
        // references the digest is immutable, so a present image is the
        // image. This also sidesteps a bollard quirk where pulling an
        // already-present digest-pinned image surfaces the daemon's
        // "cannot overwrite digest" event as a stream error.
        if docker.inspect_image(image).await.is_err() {
            pull_image(&docker, image, print).await?;
        }
        resolve_image_digest(&docker, image).await?
    };

    print.infoln(format_inner_cmd(inner, &resolved));
    run_inner_build(&docker, &resolved, inner, rsver, mount_root).await?;

    Ok(resolved)
}

async fn run_inner_build(
    docker: &Docker,
    image: &str,
    inner: &InnerBuildArgs<'_>,
    rsver: Option<&str>,
    mount_root: &Path,
) -> Result<(), Error> {
    let cargo_home = home::cargo_home().map_err(Error::CargoHome)?;
    let binds = vec![
        format!("{}:{}", mount_root.display(), SOURCE_DIR),
        format!("{}:{}", cargo_home.join("registry").display(), REGISTRY_DIR),
    ];

    let mut env = vec![
        format!("SOURCE_DATE_EPOCH={}", source_date_epoch(mount_root)),
        "CARGO_TERM_COLOR=always".to_string(),
    ];
    if let Some(t) = rsver {
        env.push(format!("RUSTUP_TOOLCHAIN={t}"));
    }

    // Override the image's entrypoint with a small shim that ensures the
    // wasm target is installed for the active rust toolchain, then exec's
    // `stellar`. Two reasons:
    //
    // - When `RUSTUP_TOOLCHAIN=<rsver>` selects a toolchain other than the
    //   image's default (typical at verify time), the image's pre-installed
    //   `wasm32v1-none` target is associated with the *other* toolchain,
    //   not the selected one — `cargo build --target=wasm32v1-none` would
    //   fail. `rustup target add` is idempotent (and quick, when the target
    //   is already present) so always running it is safe.
    // - The official `stellar/stellar-cli` image's stock entrypoint is a
    //   wrapper script that launches dbus + gnome-keyring before exec-ing
    //   `stellar`; that setup is irrelevant for `contract build` and dbus
    //   refuses to start when the container runs as a host UID with no
    //   `/etc/passwd` entry. Skipping it keeps the host UID mapping intact.
    //
    // TODO: remove this entrypoint override once
    // https://github.com/stellar/stellar-cli/issues/2545 is implemented and
    // the published image's entrypoint installs the wasm target itself
    // (and doesn't drag dbus/gnome-keyring into the contract-build path).
    let entrypoint = vec![
        "sh".to_string(),
        "-c".to_string(),
        "rustup target add wasm32v1-none --quiet && exec stellar \"$@\"".to_string(),
    ];
    let argv = build_inner_argv(inner, image);

    let config = ContainerCreateBody {
        image: Some(image.to_string()),
        entrypoint: Some(entrypoint),
        cmd: Some(argv),
        env: Some(env),
        working_dir: Some(SOURCE_DIR.to_string()),
        user: current_uid_gid(),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        host_config: Some(HostConfig {
            binds: Some(binds),
            auto_remove: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };

    let container_id = docker
        .create_container(None::<CreateContainerOptions>, config)
        .await?
        .id;

    let result = stream_and_wait(docker, &container_id).await;

    let _ = docker
        .remove_container(
            &container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;

    result
}

async fn stream_and_wait(docker: &Docker, container_id: &str) -> Result<(), Error> {
    docker
        .start_container(container_id, None::<StartContainerOptions>)
        .await?;

    let mut log_stream = docker.logs(
        container_id,
        Some(LogsOptions {
            follow: true,
            stdout: true,
            stderr: true,
            ..Default::default()
        }),
    );
    while let Some(item) = log_stream.next().await {
        let s = item?.to_string();
        let s = s.trim_end_matches('\n');
        if !s.is_empty() {
            eprintln!("{s}");
        }
    }

    let mut wait_stream = docker.wait_container(container_id, None::<WaitContainerOptions>);
    let mut exit_code: i64 = 0;
    while let Some(res) = wait_stream.next().await {
        match res {
            Ok(r) => exit_code = r.status_code,
            Err(bollard::errors::Error::DockerContainerWaitError { code, .. }) => exit_code = code,
            Err(e) => return Err(Error::Runtime(e)),
        }
    }
    if exit_code != 0 {
        return Err(Error::BuildExit(exit_code));
    }
    Ok(())
}

/// Build the argv passed via `cmd`. The image's entrypoint is overridden
/// to `sh -c '<script>' "$@"`, so the first element is the `$0` placeholder
/// for sh; the rest become `stellar`'s actual args (since `<script>` ends
/// in `exec stellar "$@"`).
///
/// We deliberately do not pass `--backend local` here: the in-container cli
/// may be a release that predates this PR and doesn't know about `--backend`.
/// Its default behavior (build locally) is what we want anyway. `bldimg` is
/// forwarded as a `--meta` entry (an existing flag) so the in-container
/// build records it in the wasm meta — without depending on any new flags
/// from this PR. The presence of `bldimg` itself signals a docker build;
/// no separate `bldbkd` field is needed.
fn build_inner_argv(inner: &InnerBuildArgs<'_>, image: &str) -> Vec<String> {
    let mut argv: Vec<String> = vec![
        "sh".to_string(), // $0 placeholder for the entrypoint's `sh -c`
        "contract".to_string(),
        "build".to_string(),
        "--manifest-path".to_string(),
        inner.manifest_path.clone(),
        "--profile".to_string(),
        inner.profile.to_string(),
        "--locked".to_string(),
        "--meta".to_string(),
        format!("bldimg={image}"),
    ];
    if let Some(p) = inner.package {
        argv.push("--package".to_string());
        argv.push(p.to_string());
    }
    if let Some(f) = inner.features {
        argv.push("--features".to_string());
        argv.push(f.to_string());
    }
    if inner.all_features {
        argv.push("--all-features".to_string());
    }
    if inner.no_default_features {
        argv.push("--no-default-features".to_string());
    }
    if inner.optimize {
        argv.push("--optimize".to_string());
    }
    for (k, v) in &inner.meta {
        argv.push("--meta".to_string());
        argv.push(format!("{k}={v}"));
    }
    argv
}

fn format_inner_cmd(inner: &InnerBuildArgs<'_>, image: &str) -> String {
    // Skip the `$0` placeholder when displaying.
    build_inner_argv(inner, image)
        .into_iter()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Pull `image` (by tag or by digest) on `linux/amd64`. Daemon-reported
/// errors in the pull event stream are surfaced as `PullDaemonError`.
pub(super) async fn pull_image(
    docker: &Docker,
    image: &str,
    print: &Print,
) -> Result<(), Error> {
    let mut stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: Some(image.to_string()),
            platform: PLATFORM.to_string(),
            ..Default::default()
        }),
        None,
        None,
    );
    let mut first = true;
    while let Some(item) = stream.try_next().await.map_err(|e| Error::ImagePull {
        image: image.to_string(),
        source: e,
    })? {
        if let Some(detail) = item.error_detail {
            return Err(Error::PullDaemonError {
                image: image.to_string(),
                message: detail.message.unwrap_or_else(|| "unknown".to_string()),
            });
        }
        if let Some(status) = item.status {
            if status.contains("Pulling from")
                || status.contains("Digest")
                || status.contains("Status")
            {
                if first {
                    print.infoln(status);
                    first = false;
                } else {
                    print.blankln(status);
                }
            }
        }
    }
    Ok(())
}

/// Returns a fully-qualified `<registry>/<path>@sha256:<digest>` reference
/// for embedding in `bldimg`. If `image` already contains an `@sha256:...`
/// reference, it's used directly. Otherwise we fall back to inspecting the
/// local image's `RepoDigests` after pull. (For the default image we ship
/// a digest-pinned reference, so this fallback is rare.)
pub(super) async fn resolve_image_digest(
    docker: &Docker,
    image: &str,
) -> Result<String, Error> {
    let canonical = fully_qualify(strip_tag(image));
    let digest = if let Some(d) = sha256_digest(image) {
        d.to_string()
    } else {
        docker
            .inspect_image(image)
            .await
            .map_err(|e| Error::ImageInspect {
                image: image.to_string(),
                source: e,
            })?
            .repo_digests
            .unwrap_or_default()
            .into_iter()
            .find_map(|d| sha256_digest(&d).map(str::to_string))
            .ok_or_else(|| Error::NoDigest {
                image: image.to_string(),
            })?
    };
    Ok(format!("{canonical}@{digest}"))
}

fn sha256_digest(image: &str) -> Option<&str> {
    let (_, after) = image.rsplit_once('@')?;
    after.starts_with("sha256:").then_some(after)
}

fn strip_tag(image: &str) -> &str {
    let no_digest = image.split_once('@').map_or(image, |(name, _)| name);
    match no_digest.rfind('/') {
        Some(slash) => match no_digest[slash + 1..].rfind(':') {
            Some(colon) => &no_digest[..slash + 1 + colon],
            None => no_digest,
        },
        None => match no_digest.rfind(':') {
            Some(colon) => &no_digest[..colon],
            None => no_digest,
        },
    }
}

fn fully_qualify(name: &str) -> String {
    let has_registry = name
        .split_once('/')
        .is_some_and(|(host, _)| host.contains('.') || host.contains(':') || host == "localhost");
    if has_registry {
        name.to_string()
    } else if name.contains('/') {
        format!("docker.io/{name}")
    } else {
        format!("docker.io/library/{name}")
    }
}

#[allow(clippy::unnecessary_wraps)]
#[cfg(unix)]
fn current_uid_gid() -> Option<String> {
    Some(format!("{}:{}", unsafe { libc::getuid() }, unsafe {
        libc::getgid()
    }))
}

#[cfg(not(unix))]
fn current_uid_gid() -> Option<String> {
    None
}

/// Best-effort SOURCE_DATE_EPOCH from the workspace's HEAD commit time;
/// falls back to `"0"` when not in a git repo.
fn source_date_epoch(mount_root: &Path) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(mount_root)
        .args(["log", "-1", "--format=%ct"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_digest_cases() {
        assert_eq!(sha256_digest("name"), None);
        assert_eq!(sha256_digest("name:tag"), None);
        assert_eq!(sha256_digest("name@md5:abc"), None);
        assert_eq!(sha256_digest("name@sha256:abc"), Some("sha256:abc"));
        assert_eq!(
            sha256_digest("host:5000/name:tag@sha256:abc"),
            Some("sha256:abc")
        );
    }

    #[test]
    fn strip_tag_cases() {
        assert_eq!(strip_tag("rust"), "rust");
        assert_eq!(strip_tag("rust:latest"), "rust");
        assert_eq!(strip_tag("rust@sha256:abc"), "rust");
        assert_eq!(strip_tag("rust:latest@sha256:abc"), "rust");
        assert_eq!(
            strip_tag("docker.io/library/rust:latest"),
            "docker.io/library/rust"
        );
        assert_eq!(strip_tag("host:5000/myimage:v1"), "host:5000/myimage");
    }

    #[test]
    fn fully_qualify_cases() {
        assert_eq!(fully_qualify("rust"), "docker.io/library/rust");
        assert_eq!(fully_qualify("myorg/myimage"), "docker.io/myorg/myimage");
        assert_eq!(
            fully_qualify("docker.io/library/rust"),
            "docker.io/library/rust"
        );
        assert_eq!(
            fully_qualify("quay.io/myorg/myimage"),
            "quay.io/myorg/myimage"
        );
        assert_eq!(fully_qualify("host:5000/myimage"), "host:5000/myimage");
        assert_eq!(fully_qualify("localhost/myimage"), "localhost/myimage");
    }
}
