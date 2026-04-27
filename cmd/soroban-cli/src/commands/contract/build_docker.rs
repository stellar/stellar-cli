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

pub const DEFAULT_IMAGE: &str = "docker.io/library/rust:latest";
const PLATFORM: &str = "linux/amd64";
pub const WORK_DIR: &str = "/work";
const TARGET_DIR: &str = "/target";
const REGISTRY_DIR: &str = "/usr/local/cargo/registry";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot connect to docker daemon; is the daemon running? ({0})")]
    DockerNotRunning(ContainerError),

    #[error("pulling docker image {image}: {source}")]
    DockerImagePull {
        image: String,
        source: bollard::errors::Error,
    },

    #[error("inspecting docker image {image}: {source}")]
    DockerImageInspect {
        image: String,
        source: bollard::errors::Error,
    },

    #[error("docker image {image} has no repository digest; pin via --docker=<registry>/<image>@sha256:...")]
    DockerNoDigest { image: String },

    #[error("build failed inside docker container (exit {0})")]
    DockerBuildExit(i64),

    #[error("docker run: {0}")]
    DockerRun(#[from] bollard::errors::Error),

    #[error("resolving CARGO_HOME: {0}")]
    CargoHome(std::io::Error),
}

/// Inputs for a single `cargo rustc` invocation inside a container.
pub struct DockerRun<'a> {
    pub workspace_root: &'a Path,
    pub target_dir: &'a Path,
    pub cargo_args: Vec<String>,
    pub env_vars: Vec<(String, String)>,
    pub image_ref: String,
    pub source_date_epoch: String,
    pub container_args: &'a ContainerArgs,
    pub print: &'a Print,
}

/// Pull (if needed), run `cargo rustc` inside the container, and return the
/// fully-qualified image reference including the resolved `@sha256:...` digest.
pub async fn run_cargo_rustc_in_docker(run: DockerRun<'_>) -> Result<String, Error> {
    let docker: Docker = match run.container_args.connect_to_docker(run.print).await {
        Ok(d) => d,
        Err(e) => return Err(Error::DockerNotRunning(e)),
    };

    pull_image(&docker, &run.image_ref, run.print).await?;
    let resolved_image = resolve_image_digest(&docker, &run.image_ref).await?;

    let cargo_home = home::cargo_home().map_err(Error::CargoHome)?;
    let registry = cargo_home.join("registry");

    let binds = vec![
        format!("{}:{}", run.workspace_root.display(), WORK_DIR),
        format!("{}:{}", run.target_dir.display(), TARGET_DIR),
        format!("{}:{}", registry.display(), REGISTRY_DIR),
    ];

    let mut env: Vec<String> = run
        .env_vars
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    env.push(format!("CARGO_TARGET_DIR={TARGET_DIR}"));
    env.push(format!("SOURCE_DATE_EPOCH={}", run.source_date_epoch));

    let mut cmd = vec!["cargo".to_string(), "rustc".to_string()];
    cmd.extend(run.cargo_args.iter().cloned());

    let user = current_uid_gid();

    let config = ContainerCreateBody {
        image: Some(resolved_image.clone()),
        cmd: Some(cmd),
        env: Some(env),
        working_dir: Some(WORK_DIR.to_string()),
        user,
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        host_config: Some(HostConfig {
            binds: Some(binds),
            network_mode: Some("none".to_string()),
            // auto_remove=false so we can stream logs, then call
            // remove_container ourselves with force=true even on failure paths.
            auto_remove: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };

    let create_resp = docker
        .create_container(None::<CreateContainerOptions>, config)
        .await?;
    let container_id = create_resp.id;

    let result = run_and_wait(&docker, &container_id, run.print).await;

    let _ = docker
        .remove_container(
            &container_id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;

    result?;

    Ok(resolved_image)
}

async fn run_and_wait(docker: &Docker, container_id: &str, print: &Print) -> Result<(), Error> {
    docker
        .start_container(container_id, None::<StartContainerOptions>)
        .await?;

    let logs_opts = LogsOptions {
        follow: true,
        stdout: true,
        stderr: true,
        ..Default::default()
    };
    let mut log_stream = docker.logs(container_id, Some(logs_opts));
    while let Some(item) = log_stream.next().await {
        match item {
            Ok(out) => {
                let s = out.to_string();
                let s = s.trim_end_matches('\n');
                if !s.is_empty() {
                    print.infoln(s);
                }
            }
            Err(e) => return Err(Error::DockerRun(e)),
        }
    }

    let mut wait_stream = docker.wait_container(container_id, None::<WaitContainerOptions>);
    let mut exit_code: i64 = 0;
    while let Some(res) = wait_stream.next().await {
        match res {
            Ok(r) => exit_code = r.status_code,
            Err(bollard::errors::Error::DockerContainerWaitError { code, .. }) => {
                exit_code = code;
            }
            Err(e) => return Err(Error::DockerRun(e)),
        }
    }
    if exit_code != 0 {
        return Err(Error::DockerBuildExit(exit_code));
    }
    Ok(())
}

async fn pull_image(docker: &Docker, image: &str, print: &Print) -> Result<(), Error> {
    let opts = CreateImageOptions {
        from_image: Some(image.to_string()),
        platform: PLATFORM.to_string(),
        ..Default::default()
    };
    let mut stream = docker.create_image(Some(opts), None, None);
    while let Some(item) = stream.try_next().await.map_err(|e| Error::DockerImagePull {
        image: image.to_string(),
        source: e,
    })? {
        if let Some(status) = item.status {
            if status.contains("Pulling from") || status.contains("Digest") || status.contains("Status") {
                print.infoln(status);
            }
        }
    }
    Ok(())
}

async fn resolve_image_digest(docker: &Docker, image: &str) -> Result<String, Error> {
    if let Some((name, digest)) = parse_pinned_digest(image) {
        return Ok(format!("{name}@{digest}"));
    }
    let info = docker
        .inspect_image(image)
        .await
        .map_err(|e| Error::DockerImageInspect {
            image: image.to_string(),
            source: e,
        })?;
    let repo_digests = info.repo_digests.unwrap_or_default();
    // The digest is the platform-specific manifest digest because we pulled
    // with `--platform=linux/amd64`; reproducibility on `verify` depends on
    // always pulling with that same platform.
    //
    // If the daemon has multiple repo_digests (e.g. registry mirror configs),
    // prefer the one whose name prefix matches the user-supplied image.
    let want_prefix = strip_tag(image);
    let chosen = repo_digests
        .iter()
        .find(|d| strip_digest(d) == want_prefix)
        .cloned()
        .or_else(|| repo_digests.first().cloned())
        .ok_or_else(|| Error::DockerNoDigest {
            image: image.to_string(),
        })?;
    Ok(chosen)
}

/// Parse a `name@sha256:...` reference into `(name, digest)`. Returns `None`
/// if the reference does not contain a `@sha256:` digest.
fn parse_pinned_digest(image: &str) -> Option<(&str, &str)> {
    let (name, after) = image.rsplit_once('@')?;
    if after.starts_with("sha256:") {
        Some((name, after))
    } else {
        None
    }
}

fn strip_digest(image: &str) -> &str {
    image.split_once('@').map_or(image, |(name, _)| name)
}

/// Strip both `@sha256:...` and `:tag`, leaving just the repository name.
fn strip_tag(image: &str) -> &str {
    let no_digest = strip_digest(image);
    // A `:` in the host portion (e.g. `host:5000/name`) is not a tag separator.
    // Tags only appear after the last `/`.
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

#[allow(clippy::unnecessary_wraps)]
#[cfg(unix)]
fn current_uid_gid() -> Option<String> {
    // SAFETY: getuid/getgid are infallible POSIX calls returning the real
    // user/group ID of the calling process.
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    Some(format!("{uid}:{gid}"))
}

#[cfg(not(unix))]
fn current_uid_gid() -> Option<String> {
    None
}

/// Build the equivalent `docker run ...` command line for `--print-commands-only`.
pub fn print_docker_command(
    workspace_root: &Path,
    target_dir: &Path,
    cargo_args: &[String],
    env_vars: &[(String, String)],
    image_ref: &str,
    source_date_epoch: &str,
) -> Result<String, Error> {
    let cargo_home = home::cargo_home().map_err(Error::CargoHome)?;
    let registry = cargo_home.join("registry");

    let mut parts: Vec<String> = vec![
        "docker".to_string(),
        "run".to_string(),
        "--rm".to_string(),
        format!("--platform={PLATFORM}"),
        "--network=none".to_string(),
        format!("-w {WORK_DIR}"),
    ];

    if let Some(user) = current_uid_gid() {
        parts.push(format!("-u {user}"));
    }

    parts.push(shell_escape_kv(
        "-v",
        &format!("{}:{}", workspace_root.display(), WORK_DIR),
    ));
    parts.push(shell_escape_kv(
        "-v",
        &format!("{}:{}", target_dir.display(), TARGET_DIR),
    ));
    parts.push(shell_escape_kv(
        "-v",
        &format!("{}:{}", registry.display(), REGISTRY_DIR),
    ));

    for (k, v) in env_vars {
        parts.push(shell_escape_kv("-e", &format!("{k}={v}")));
    }
    parts.push(shell_escape_kv(
        "-e",
        &format!("CARGO_TARGET_DIR={TARGET_DIR}"),
    ));
    parts.push(shell_escape_kv(
        "-e",
        &format!("SOURCE_DATE_EPOCH={source_date_epoch}"),
    ));

    parts.push(shell_escape::escape(image_ref.into()).into_owned());
    parts.push("cargo".to_string());
    parts.push("rustc".to_string());
    for a in cargo_args {
        parts.push(shell_escape::escape(a.into()).into_owned());
    }

    Ok(parts.join(" "))
}

fn shell_escape_kv(flag: &str, value: &str) -> String {
    format!(
        "{flag} {}",
        shell_escape::escape(value.into()).into_owned()
    )
}

/// Best-effort SOURCE_DATE_EPOCH derived from the workspace's HEAD commit time.
/// Falls back to `"0"` when not in a git repo or git is unavailable.
pub fn source_date_epoch(workspace_root: &Path) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(["log", "-1", "--format=%ct"])
        .output();
    if let Ok(out) = output {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    "0".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pinned_digest_cases() {
        assert_eq!(parse_pinned_digest("name"), None);
        assert_eq!(parse_pinned_digest("name:tag"), None);
        assert_eq!(
            parse_pinned_digest("name@sha256:abc"),
            Some(("name", "sha256:abc"))
        );
        assert_eq!(
            parse_pinned_digest("host/path/name@sha256:abc"),
            Some(("host/path/name", "sha256:abc"))
        );
        assert_eq!(
            parse_pinned_digest("host:5000/name:tag@sha256:abc"),
            Some(("host:5000/name:tag", "sha256:abc"))
        );
        // Non-sha256 algorithms are not recognized.
        assert_eq!(parse_pinned_digest("name@md5:abc"), None);
    }

    #[test]
    fn strip_digest_cases() {
        assert_eq!(strip_digest("name"), "name");
        assert_eq!(strip_digest("name:tag"), "name:tag");
        assert_eq!(strip_digest("name@sha256:abc"), "name");
        assert_eq!(
            strip_digest("host/name:tag@sha256:abc"),
            "host/name:tag"
        );
    }

    #[test]
    fn strip_tag_cases() {
        assert_eq!(strip_tag("name"), "name");
        assert_eq!(strip_tag("name:tag"), "name");
        assert_eq!(strip_tag("host/name:tag"), "host/name");
        assert_eq!(strip_tag("host:5000/name"), "host:5000/name");
        assert_eq!(strip_tag("host:5000/name:tag"), "host:5000/name");
        assert_eq!(strip_tag("name@sha256:abc"), "name");
        assert_eq!(strip_tag("host:5000/name:tag@sha256:abc"), "host:5000/name");
    }
}
