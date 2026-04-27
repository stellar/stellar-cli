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

/// Pull (if needed), run the host `cmd` (its program and args) inside a
/// linux/amd64 container, and return the resolved `name@sha256:...` reference.
pub async fn run_in_docker(
    cmd: &Command,
    image: &str,
    workspace_root: &Path,
    target_dir: &Path,
    container_args: &ContainerArgs,
    print: &Print,
) -> Result<String, Error> {
    let docker: Docker = container_args
        .connect_to_docker(print)
        .await
        .map_err(Error::DockerNotRunning)?;

    pull_image(&docker, image, print).await?;
    let resolved = resolve_image_digest(&docker, image).await?;

    let cargo_home = home::cargo_home().map_err(Error::CargoHome)?;
    let binds = vec![
        format!("{}:{}", workspace_root.display(), WORK_DIR),
        format!("{}:{}", target_dir.display(), TARGET_DIR),
        format!("{}:{}", cargo_home.join("registry").display(), REGISTRY_DIR),
    ];

    let mut env: Vec<String> = cmd
        .get_envs()
        .filter_map(|(k, v)| {
            v.map(|val| format!("{}={}", k.to_string_lossy(), val.to_string_lossy()))
        })
        .collect();
    env.push(format!("CARGO_TARGET_DIR={TARGET_DIR}"));
    env.push(format!("SOURCE_DATE_EPOCH={}", source_date_epoch(workspace_root)));

    let container_cmd: Vec<String> = std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(OsStr::to_string_lossy)
        .map(std::borrow::Cow::into_owned)
        .collect();

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
            network_mode: Some("none".to_string()),
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

    let result = run_and_wait(&docker, &container_id, print).await;

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

async fn run_and_wait(docker: &Docker, container_id: &str, print: &Print) -> Result<(), Error> {
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
            print.infoln(s);
        }
    }

    let mut wait_stream = docker.wait_container(container_id, None::<WaitContainerOptions>);
    let mut exit_code: i64 = 0;
    while let Some(res) = wait_stream.next().await {
        match res {
            Ok(r) => exit_code = r.status_code,
            Err(bollard::errors::Error::DockerContainerWaitError { code, .. }) => exit_code = code,
            Err(e) => return Err(Error::DockerRun(e)),
        }
    }
    if exit_code != 0 {
        return Err(Error::DockerBuildExit(exit_code));
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

// We pull with --platform=linux/amd64 so the recorded digest is platform-specific;
// reproducibility on `verify` depends on always pulling with that same platform.
async fn resolve_image_digest(docker: &Docker, image: &str) -> Result<String, Error> {
    if parse_pinned_digest(image).is_some() {
        return Ok(image.to_string());
    }
    let info = docker
        .inspect_image(image)
        .await
        .map_err(|e| Error::DockerImageInspect {
            image: image.to_string(),
            source: e,
        })?;
    info.repo_digests
        .unwrap_or_default()
        .into_iter()
        .next()
        .ok_or_else(|| Error::DockerNoDigest {
            image: image.to_string(),
        })
}

fn parse_pinned_digest(image: &str) -> Option<(&str, &str)> {
    let (name, after) = image.rsplit_once('@')?;
    after.starts_with("sha256:").then_some((name, after))
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
    fn parse_pinned_digest_cases() {
        assert_eq!(parse_pinned_digest("name"), None);
        assert_eq!(parse_pinned_digest("name:tag"), None);
        assert_eq!(parse_pinned_digest("name@md5:abc"), None);
        assert_eq!(
            parse_pinned_digest("name@sha256:abc"),
            Some(("name", "sha256:abc"))
        );
        assert_eq!(
            parse_pinned_digest("host:5000/name:tag@sha256:abc"),
            Some(("host:5000/name:tag", "sha256:abc"))
        );
    }
}
