//! `--backend docker-all` build backend.
//!
//! Layers a stellar-cli install on top of the user's chosen rust base image,
//! then runs the entire `stellar contract build --backend local` pipeline
//! inside the container — including the post-build steps (meta injection,
//! spec filtering, optimization). The host-side stellar-cli only orchestrates.
//!
//! Reproducibility: `bldimg` records the *base* rust image digest (same as
//! `--backend docker`), `cliver` records the stellar-cli version installed
//! into the layered image, and `rsver` is recorded by soroban-sdk. Together
//! these three reconstruct the layered image on `verify`.

use std::collections::HashMap;
use std::path::Path;

use bollard::{
    models::ContainerCreateBody,
    query_parameters::{
        BuildImageOptionsBuilder, CreateContainerOptions, LogsOptions, RemoveContainerOptions,
        StartContainerOptions, WaitContainerOptions,
    },
    service::HostConfig,
    Docker,
};
use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::{Either, Full};

use super::build_docker::{
    current_uid_gid, pull_image, resolve_image_digest, source_date_epoch, Error,
};
use crate::{commands::container::shared::Args as ContainerArgs, print::Print};

const DOCKERFILE: &str = include_str!("build_docker_all/Dockerfile");
const STELLAR_CLI_REPO: &str = "https://github.com/stellar/stellar-cli";

/// Where the workspace and target are mounted inside the container, and where
/// the cargo registry cache lives. The first two are bind mounts shared with
/// the host; the registry cache is a bind mount of the host's cargo registry.
const WORK_DIR: &str = "/workspace";
const TARGET_DIR: &str = "/target";
const REGISTRY_DIR: &str = "/usr/local/cargo/registry";
const PLATFORM: &str = "linux/amd64";

/// Forwarded host build args used to construct the inner
/// `stellar contract build --backend local` invocation. `manifest_path` is
/// expected to already be in container-relative form (`/workspace/...`).
pub struct InnerBuildArgs<'a> {
    pub manifest_path: String,
    pub package: Option<&'a str>,
    pub profile: &'a str,
    pub features: Option<&'a str>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub optimize: bool,
    pub meta: &'a [(String, String)],
    pub rustup_toolchain: Option<&'a str>,
}

/// Pull the base image, build a layered stellar-cli image on top of it, then
/// run `stellar contract build --backend local --bldimg <base> --bldbkd docker-all`
/// inside that layered image. Returns the resolved base image reference for
/// embedding into `bldimg`.
#[allow(clippy::too_many_arguments)]
pub async fn run_in_docker_all(
    base_image: &str,
    cli_rev: &str,
    mount_root: &Path,
    target_dir: &Path,
    wasm_target: &str,
    inner: &InnerBuildArgs<'_>,
    container_args: &ContainerArgs,
    print: &Print,
) -> Result<String, Error> {
    let docker: Docker = container_args
        .connect_to_docker(print)
        .await
        .map_err(Error::RuntimeNotRunning)?;

    pull_image(&docker, base_image, print).await?;
    let base_resolved = resolve_image_digest(&docker, base_image).await?;

    let layered_tag = format!("stellar-cli-build:{}", short_hash(&base_resolved, cli_rev));
    print.infoln(format!(
        "Building stellar-cli build image {layered_tag} (base {base_resolved}, stellar-cli {cli_rev})"
    ));
    build_layered_image(
        &docker,
        &base_resolved,
        cli_rev,
        wasm_target,
        &layered_tag,
        print,
    )
    .await?;

    print.infoln(format_inner_cmd(inner, &base_resolved));

    run_inner_build(
        &docker,
        &layered_tag,
        &base_resolved,
        inner,
        mount_root,
        target_dir,
    )
    .await?;

    Ok(base_resolved)
}

/// Build the layered image (FROM base + rustup target + cargo install stellar-cli).
async fn build_layered_image(
    docker: &Docker,
    base_image: &str,
    cli_rev: &str,
    wasm_target: &str,
    tag: &str,
    print: &Print,
) -> Result<(), Error> {
    let context = build_tar_context()?;

    let mut buildargs: HashMap<String, String> = HashMap::new();
    buildargs.insert("BASE_IMAGE".to_string(), base_image.to_string());
    buildargs.insert("WASM_TARGET".to_string(), wasm_target.to_string());
    buildargs.insert(
        "STELLAR_CLI_REPO".to_string(),
        STELLAR_CLI_REPO.to_string(),
    );
    buildargs.insert("STELLAR_CLI_REV".to_string(), cli_rev.to_string());

    let options = BuildImageOptionsBuilder::default()
        .dockerfile("Dockerfile")
        .t(tag)
        .platform(PLATFORM)
        .buildargs(&buildargs)
        .rm(true)
        .build();

    let body = Either::Left(Full::new(context));
    let mut stream = docker.build_image(options, None, Some(body));
    while let Some(item) = stream.next().await {
        let info = item?;
        if let Some(s) = info.stream {
            let s = s.trim_end_matches('\n');
            if !s.is_empty() {
                print.blankln(s);
            }
        }
        if let Some(detail) = info.error_detail {
            return Err(Error::ImageBuild(
                detail.message.unwrap_or_else(|| "unknown".to_string()),
            ));
        }
    }
    Ok(())
}

/// Construct an in-memory tar containing only the embedded Dockerfile.
fn build_tar_context() -> Result<Bytes, Error> {
    let dockerfile = DOCKERFILE.as_bytes();
    let mut buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut buf);
        let mut header = tar::Header::new_gnu();
        header.set_path("Dockerfile").map_err(Error::Tar)?;
        header.set_size(dockerfile.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, dockerfile).map_err(Error::Tar)?;
        builder.finish().map_err(Error::Tar)?;
    }
    Ok(Bytes::from(buf))
}

/// Run the in-container `stellar contract build --backend local --bldimg ... --bldbkd docker-all`.
async fn run_inner_build(
    docker: &Docker,
    layered_tag: &str,
    base_resolved: &str,
    inner: &InnerBuildArgs<'_>,
    mount_root: &Path,
    target_dir: &Path,
) -> Result<(), Error> {
    let cargo_home = home::cargo_home().map_err(Error::CargoHome)?;
    let binds = vec![
        format!("{}:{}", mount_root.display(), WORK_DIR),
        format!("{}:{}", target_dir.display(), TARGET_DIR),
        format!("{}:{}", cargo_home.join("registry").display(), REGISTRY_DIR),
    ];

    let env = vec![
        format!("CARGO_TARGET_DIR={TARGET_DIR}"),
        format!("SOURCE_DATE_EPOCH={}", source_date_epoch(mount_root)),
        "CARGO_TERM_COLOR=always".to_string(),
    ];

    let argv = build_inner_argv(inner, base_resolved);

    let config = ContainerCreateBody {
        image: Some(layered_tag.to_string()),
        cmd: Some(argv),
        env: Some(env),
        working_dir: Some(WORK_DIR.to_string()),
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

/// Build the argv for the in-container `stellar contract build --backend local`.
fn build_inner_argv(inner: &InnerBuildArgs<'_>, base_resolved: &str) -> Vec<String> {
    let mut argv: Vec<String> = vec![
        "stellar".to_string(),
        "contract".to_string(),
        "build".to_string(),
        "--backend".to_string(),
        "local".to_string(),
        "--bldimg".to_string(),
        base_resolved.to_string(),
        "--bldbkd".to_string(),
        "docker-all".to_string(),
        "--manifest-path".to_string(),
        inner.manifest_path.clone(),
        "--profile".to_string(),
        inner.profile.to_string(),
        // Always --locked so the inner build is deterministic.
        "--locked".to_string(),
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
    for (k, v) in inner.meta {
        argv.push("--meta".to_string());
        argv.push(format!("{k}={v}"));
    }
    if let Some(t) = inner.rustup_toolchain {
        argv.push("--rustup-toolchain".to_string());
        argv.push(t.to_string());
    }
    argv
}

/// Stable short tag suffix from `(base_image, cli_rev)`.
fn short_hash(base_image: &str, cli_rev: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(base_image.as_bytes());
    h.update(b"\0");
    h.update(cli_rev.as_bytes());
    let digest = h.finalize();
    hex::encode(&digest[..8])
}

/// One-line preview of the inner cargo command, for the `ℹ︎ ...` info log.
fn format_inner_cmd(inner: &InnerBuildArgs<'_>, base_resolved: &str) -> String {
    build_inner_argv(inner, base_resolved).join(" ")
}

/// Reduce a host stellar-cli git revision string to a 40-char commit sha
/// plus a dirty flag.
///
/// `crate_git_revision` (stellar's fork) emits one of two shapes:
/// - `<40-char-sha>` for a clean working tree
/// - `<40-char-sha>-dirty` for a working tree with uncommitted changes
///
/// The bare sha is what `cargo install --git --rev` needs. Callers should
/// warn the user when the dirty flag is set: the layered image will install
/// the *clean* commit, so the resulting wasm won't match what a clean host
/// CLI would have produced.
pub fn extract_full_sha(git: &str) -> Result<(String, bool), Error> {
    if git.is_empty() {
        return Err(Error::NoHostCliRev);
    }
    let (sha, dirty) = match git.strip_suffix("-dirty") {
        Some(s) => (s, true),
        None => (git, false),
    };
    if is_full_sha(sha) {
        Ok((sha.to_string(), dirty))
    } else {
        Err(Error::NoHostCliRev)
    }
}

fn is_full_sha(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_full_sha_clean() {
        let sha = "60f7458e7ecffddf2f2d91dc6d0d2db4fab03ecc";
        assert_eq!(extract_full_sha(sha).unwrap(), (sha.to_string(), false));
    }

    #[test]
    fn extract_full_sha_dirty() {
        let sha = "edc5397642bb6e53a4eb6c96348493df105ffa69";
        let input = format!("{sha}-dirty");
        assert_eq!(
            extract_full_sha(&input).unwrap(),
            (sha.to_string(), true)
        );
    }

    #[test]
    fn extract_full_sha_empty_errors() {
        assert!(matches!(extract_full_sha(""), Err(Error::NoHostCliRev)));
    }

    #[test]
    fn extract_full_sha_short_errors() {
        assert!(matches!(extract_full_sha("abc"), Err(Error::NoHostCliRev)));
    }

    #[test]
    fn extract_full_sha_describe_form_no_longer_supported() {
        // `crate_git_revision` (stellar's fork) emits only `<sha>` or
        // `<sha>-dirty` now; the legacy git-describe form should error.
        let s = "v20.0.0-836-gfe07b3678833e07c43235a6caaeccff81e146856";
        assert!(matches!(extract_full_sha(s), Err(Error::NoHostCliRev)));
    }

    #[test]
    fn short_hash_is_deterministic_and_short() {
        let a = short_hash("docker.io/library/rust@sha256:abc", "deadbeef");
        let b = short_hash("docker.io/library/rust@sha256:abc", "deadbeef");
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }
}
