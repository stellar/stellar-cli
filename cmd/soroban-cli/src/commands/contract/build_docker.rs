use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

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
pub const WORK_DIR: &str = "/workspace";
const TARGET_DIR: &str = "/target";
const REGISTRY_DIR: &str = "/usr/local/cargo/registry";
const RUSTUP_DIR: &str = "/usr/local/rustup";

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

    #[error("docker image {image} has no repository digest; pin via --backend docker=<registry>/<image>@sha256:...")]
    NoDigest { image: String },

    #[error("build failed inside docker container (exit {0})")]
    BuildExit(i64),

    #[error("docker run: {0}")]
    Runtime(#[from] bollard::errors::Error),

    #[error("resolving CARGO_HOME / RUSTUP_HOME: {0}")]
    CargoHome(std::io::Error),
}

/// Pull (if needed) and run the host `cmd` inside a linux/amd64 container,
/// returning the resolved `name@sha256:...` reference for embedding into meta.
#[allow(clippy::too_many_arguments)]
pub async fn run_in_docker(
    cmd: &Command,
    cmd_str: &str,
    image: &str,
    workspace_root: &Path,
    target_dir: &Path,
    wasm_target: &str,
    pin_toolchain: Option<&str>,
    container_args: &ContainerArgs,
    print: &Print,
) -> Result<String, Error> {
    let docker: Docker = container_args
        .connect_to_docker(print)
        .await
        .map_err(Error::RuntimeNotRunning)?;

    pull_image(&docker, image, print).await?;
    let resolved = resolve_image_digest(&docker, image).await?;
    // Print the cargo invocation after the pull progress so the on-screen
    // order matches execution: pull → cargo → cargo output.
    print.infoln(cmd_str);

    // Bind-mount the host's cargo registry and rustup state. Bind mounts
    // preserve host ownership, so the container (running as the host user)
    // can write to them. This caches crate downloads and installed
    // toolchains across runs.
    let cargo_home = home::cargo_home().map_err(Error::CargoHome)?;
    let rustup_home = home::rustup_home().map_err(Error::CargoHome)?;
    let binds = vec![
        format!("{}:{}", workspace_root.display(), WORK_DIR),
        format!("{}:{}", target_dir.display(), TARGET_DIR),
        format!("{}:{}", cargo_home.join("registry").display(), REGISTRY_DIR),
        format!("{}:{}", rustup_home.display(), RUSTUP_DIR),
    ];

    let mut env: Vec<String> = cmd
        .get_envs()
        .filter_map(|(k, v)| {
            v.map(|val| format!("{}={}", k.to_string_lossy(), val.to_string_lossy()))
        })
        .collect();
    env.push(format!("CARGO_TARGET_DIR={TARGET_DIR}"));
    env.push(format!(
        "SOURCE_DATE_EPOCH={}",
        source_date_epoch(workspace_root)
    ));
    // Force cargo to emit color (otherwise cargo detects the non-TTY stdout
    // and falls back to monochrome). Matches what users see for local builds.
    env.push("CARGO_TERM_COLOR=always".to_string());

    let argv: Vec<String> = std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(OsStr::to_string_lossy)
        .map(std::borrow::Cow::into_owned)
        .collect();
    // Always install the wasm target before the build so we don't depend on
    // the workspace's `rust-toolchain.toml` having configured it. Args pass
    // through `$@` so we don't have to shell-escape.
    let toolchain_arg = pin_toolchain
        .map(|t| format!("--toolchain {t} "))
        .unwrap_or_default();
    let mut container_cmd = vec![
        "sh".to_string(),
        "-c".to_string(),
        format!("rustup --quiet target add {toolchain_arg}{wasm_target} && exec \"$@\""),
        "sh".to_string(),
    ];
    container_cmd.extend(argv);

    let config = ContainerCreateBody {
        image: Some(resolved.clone()),
        cmd: Some(container_cmd),
        env: Some(env),
        working_dir: Some(WORK_DIR.to_string()),
        user: current_uid_gid(),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        host_config: Some(HostConfig {
            binds: Some(binds),
            // auto_remove=false so we can stream logs first, then call
            // remove_container ourselves with force=true even on failure paths.
            auto_remove: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };

    let container_id = docker
        .create_container(None::<CreateContainerOptions>, config)
        .await?
        .id;

    let result = run_and_wait(&docker, &container_id).await;

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
    Ok(resolved)
}

async fn run_and_wait(docker: &Docker, container_id: &str) -> Result<(), Error> {
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
            // Emit container output raw (no `ℹ️` prefix) so it looks like
            // cargo running locally.
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

async fn pull_image(docker: &Docker, image: &str, print: &Print) -> Result<(), Error> {
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

// We pull with --platform=linux/amd64 so the recorded digest is platform-specific;
// reproducibility on `verify` depends on always pulling with that same platform.
// Returns a fully-qualified `<registry>/<path>@sha256:<digest>` reference so
// that `verify` on a different machine can resolve it without depending on
// local registry config.
async fn resolve_image_digest(docker: &Docker, image: &str) -> Result<String, Error> {
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

/// Returns the `sha256:...` portion of a `<name>@sha256:...` reference, if present.
fn sha256_digest(image: &str) -> Option<&str> {
    let (_, after) = image.rsplit_once('@')?;
    after.starts_with("sha256:").then_some(after)
}

/// Strip any `@sha256:...` and `:tag` suffix, leaving only the repository name.
fn strip_tag(image: &str) -> &str {
    let no_digest = image.split_once('@').map_or(image, |(name, _)| name);
    // Tags appear after the last `/`; a `:` in the host portion (host:port) is not a tag.
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

/// Add the implicit `docker.io` registry (and `library/` namespace for short names).
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
    // SAFETY: getuid/getgid are infallible POSIX calls.
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
fn source_date_epoch(workspace_root: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(workspace_root)
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
