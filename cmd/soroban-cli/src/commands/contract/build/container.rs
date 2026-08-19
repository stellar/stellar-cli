//! Build a contract inside a container image.
//!
//! Triggered by `stellar contract build --image <ref>`: instead of compiling
//! locally, the working tree is bind-mounted into the given container image at
//! `/source` and `stellar contract build` is run there. The resulting wasm is
//! written into the mounted `target/` directory and therefore lands on the host
//! directly. Any image ref is accepted — a tag (`:latest`) or a digest.
//!
//! This is deliberately standalone: no source archive, no clean-git-tree
//! requirement, no reproducibility metadata. It reuses the container engine
//! abstraction in [`crate::commands::container::shared`], so `--engine`,
//! `--docker-host`, and the default engine set by `stellar container use` all
//! apply.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use cargo_metadata::MetadataCommand;
use semver::Version;

use crate::commands::{container::shared, global};
use crate::print::Print;

use super::{get_wasm_target, BuiltContract, Cmd, WASM_TARGET, WASM_TARGET_OLD};

/// First CLI release whose `contract build` accepts `--locked` (added in cli
/// v25.2.0). Older images reject it, so it's dropped (with a warning) on anything
/// older, matching the version detected from the image's own `version` output.
const LOCKED_MIN: &str = "25.2.0";

/// First CLI release whose `contract build` has the `--optimize` flag at all.
/// Older images reject it, so — since optimization is on by default — this is the
/// effective minimum supported image. We probe the image's `version` and skip the
/// flag (with a warning) on anything older.
const OPTIMIZE_FLAG_MIN: &str = "23.2.0";

/// First CLI release whose `contract build` accepts `--optimize=false` as an
/// explicit value. Images between [`OPTIMIZE_FLAG_MIN`] and this default to *not*
/// optimizing, so for them we forward nothing to get an unoptimized build.
const OPTIMIZE_NEW_SYNTAX_MIN: &str = "26.1.0";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Engine(#[from] shared::Error),

    #[error("could not pull image {image}")]
    PullImageFailed { image: String },

    #[error(
        "could not determine the image's default Rust toolchain via `rustup default`; \
         the image must provide rustup so the build toolchain can be pinned"
    )]
    ToolchainProbeFailed,

    #[error("cargo metadata failed: {0}")]
    Metadata(#[from] cargo_metadata::Error),

    #[error("container build exited with status {status}. To reproduce manually:\n  {command}")]
    ContainerExit { status: i64, command: String },

    #[error("build interrupted; stopped the build container")]
    Interrupted,
}

pub async fn run(
    cmd: &Cmd,
    _global_args: &global::Args,
    print: &Print,
) -> Result<Vec<BuiltContract>, super::Error> {
    let image = cmd
        .image
        .as_deref()
        .expect("container::run is only called when --image is set");

    let docker = cmd.container_args.clone();
    docker.warn_if_host_ignored(print);

    // Bind-mount the workspace root so every crate is available and relative
    // manifest paths resolve inside the container. `cargo metadata` is resolved
    // once here and reused for workspace root, package selection, and artifact
    // collection, so a large or networked workspace pays a single subprocess.
    let md = metadata(cmd).map_err(Error::from)?;
    let workspace_root = md.workspace_root.clone().into_std_path_buf();

    // With `--print-commands-only` nothing runs, so don't pull, probe, or build;
    // just render the run command against a current image below.
    let print_only = cmd.print_commands_only;

    // By default the build uses the image already present locally and doesn't
    // pull, matching `docker run` (whose default `--pull=missing` only fetches a
    // *missing* image, never re-pulling an existing tag). This keeps a
    // locally-built or digest-pinned image as-is. `--pull` opts in to an explicit
    // `pull` up front to refresh a moving tag to its newest image. Nothing is
    // pulled when only printing the command, since nothing runs.
    if !print_only && cmd.pull {
        pull_image(&docker, image, print).await?;
    }

    // Gather everything we need to know about the image in one throwaway
    // container (binary name, CLI version, default rustup toolchain), so slow or
    // remote engines pay a single round-trip instead of one per fact. When only
    // printing the command we can't probe (that would run a container), so a
    // current image is assumed and the toolchain pin is omitted.
    let probe = if print_only {
        None
    } else {
        Some(probe_image(image, &docker).await?)
    };
    // The CLI version drives flag gating; a probed image that didn't report a
    // parseable version is treated as current (with a warning).
    let cli_version = match &probe {
        Some(p) if p.version.is_none() => {
            print.warnln("Could not probe container cli version; assuming a current image");
            None
        }
        Some(p) => p.version.clone(),
        None => None,
    };
    let at_least = |min: &str| {
        cli_version
            .as_ref()
            .is_none_or(|v| *v >= Version::parse(min).unwrap())
    };
    // `--locked` was added in v25.2.0, the `--optimize` flag in v23.2.0, and its
    // explicit `--optimize=false` value in v26.1.0.
    let supports_locked = at_least(LOCKED_MIN);
    let supports_optimize_flag = at_least(OPTIMIZE_FLAG_MIN);
    let supports_optimize_false = at_least(OPTIMIZE_NEW_SYNTAX_MIN);
    if cmd.locked && !supports_locked {
        print.warnln(
            "The build image's `contract build` does not support --locked; \
             building without it.",
        );
    }
    if cmd.build_args.optimize && !supports_optimize_flag {
        print.warnln(format!(
            "The build image's `contract build` does not support --optimize \
             (added in cli v{OPTIMIZE_FLAG_MIN}); building without optimization.",
        ));
    }

    // Build once per package so workspaces with several cdylibs all get built;
    // an explicit `--package` wins, otherwise the default-member cdylibs are
    // inferred exactly like a local build.
    let packages = resolve_packages(cmd, &md);
    if cmd.package.is_none() && !packages.is_empty() {
        print.infoln(format!("Building packages: {}", packages.join(", ")));
    }
    let targets: Vec<Option<&str>> = if packages.is_empty() {
        vec![None]
    } else {
        packages.iter().map(|p| Some(p.as_str())).collect()
    };
    let container_cmds: Vec<Vec<String>> = targets
        .iter()
        .map(|target| {
            forwarded_build_args(
                cmd,
                &workspace_root,
                *target,
                supports_locked,
                supports_optimize_flag,
                supports_optimize_false,
            )
        })
        .collect();

    // Reset the target dir to a known location under the mount, independent of
    // any mounted `.cargo/config` `build.target-dir` or image env, so we always
    // know where to collect artifacts.
    let mut env: Vec<String> = vec!["CARGO_TARGET_DIR=/source/target".to_string()];

    // Pin RUSTUP_TOOLCHAIN to the image's own default toolchain so a
    // `rust-toolchain.toml` in the mounted source can't redirect the build to a
    // different toolchain — which rustup would then try to install (needing
    // network access and possibly lacking the wasm target). An empty
    // RUSTUP_TOOLCHAIN would *not* achieve this: rustup treats it as unset and
    // still honors rust-toolchain.toml, so the probe reports the concrete
    // toolchain name (guaranteed non-empty; `probe_image` hard-fails otherwise).
    // Skipped when only printing the command, where nothing is probed.
    if let Some(p) = &probe {
        print.infoln(format!("Using Rust toolchain {}", p.toolchain));
        env.push(format!("RUSTUP_TOOLCHAIN={}", p.toolchain));
    }

    // Chaining several builds through `/bin/sh` invokes the CLI by name, which
    // differs across images (`soroban` before v21.0.0, `stellar` since). The
    // single-build path uses the image's entrypoint and doesn't care. Default to
    // `stellar` when not probed (print-only).
    let bin = probe
        .as_ref()
        .map_or_else(|| "stellar".to_string(), |p| p.bin.clone());

    run_in_container(
        image,
        &workspace_root,
        &container_cmds,
        &env,
        &docker,
        &cmd.run_args,
        &bin,
        print,
        print_only,
    )
    .await?;

    // Nothing was built when only printing the command.
    if print_only {
        return Ok(Vec::new());
    }

    collect_built_contracts(cmd, &md, &workspace_root)
}

fn metadata(cmd: &Cmd) -> Result<cargo_metadata::Metadata, cargo_metadata::Error> {
    let mut mc = MetadataCommand::new();
    mc.no_deps();
    if let Some(p) = &cmd.manifest_path {
        mc.manifest_path(p);
    }
    mc.exec()
}

/// Resolve the packages to build. An explicit `--package` wins; otherwise the
/// default-member crates that build a cdylib, mirroring the local build's
/// package selection. May be empty (no cdylib default members), in which case
/// the caller falls back to a single no-`--package` build.
fn resolve_packages(cmd: &Cmd, md: &cargo_metadata::Metadata) -> Vec<String> {
    if let Some(pkg) = &cmd.package {
        return vec![pkg.clone()];
    }
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
    names
}

/// The `contract build …` argv forwarded to the container, mirroring the local
/// build's flags. `--manifest-path` is relativized against the workspace root so
/// it's valid inside `/source`. `--out-dir` is deliberately omitted — artifacts
/// are collected on the host from the mounted `target/`.
///
/// `supports_locked`: whether the container's `contract build` accepts `--locked`
/// (added in cli 25.2.0). When false, the user's `--locked` is dropped rather
/// than forwarded to an image that would reject it.
///
/// `supports_optimize_flag`: whether the container's cli has the `--optimize`
/// flag at all (added in cli 23.2.0). When false, nothing about optimize is
/// forwarded — the flag would be rejected as unknown.
///
/// `supports_optimize_false`: whether the container's cli accepts
/// `--optimize=false` (added in cli 26.1.0). When false and the user disabled
/// optimization, nothing is forwarded — the older cli defaults to not
/// optimizing, and passing `--optimize=false` there would fail.
fn forwarded_build_args(
    cmd: &Cmd,
    workspace_root: &Path,
    package: Option<&str>,
    supports_locked: bool,
    supports_optimize_flag: bool,
    supports_optimize_false: bool,
) -> Vec<String> {
    let mut args = vec!["contract".to_string(), "build".to_string()];

    if cmd.locked && supports_locked {
        args.push("--locked".to_string());
    }
    if let Some(path) = &cmd.manifest_path {
        let abs = std::path::absolute(path).unwrap_or_else(|_| path.clone());
        let rel = abs
            .strip_prefix(workspace_root)
            .map(Path::to_path_buf)
            .unwrap_or(abs);
        // The path is forwarded to `cargo` inside a Linux container, so it must
        // use `/` separators even when this CLI is built and run on Windows.
        args.push(format!(
            "--manifest-path={}",
            rel.display().to_string().replace('\\', "/")
        ));
    }
    if cmd.profile != "release" {
        args.push(format!("--profile={}", cmd.profile));
    }
    if let Some(features) = &cmd.features {
        args.push(format!("--features={features}"));
    }
    if cmd.all_features {
        args.push("--all-features".to_string());
    }
    if cmd.no_default_features {
        args.push("--no-default-features".to_string());
    }
    if let Some(pkg) = package {
        args.push(format!("--package={pkg}"));
    }
    for (k, v) in &cmd.build_args.meta {
        args.push(format!("--meta={k}={v}"));
    }
    // Optimization is forwarded per the image's cli version. To enable it, bare
    // `--optimize` on images >= v23.2.0 (older images lack the flag entirely, so
    // forward nothing). To disable it, `--optimize=false` on images >= v26.1.0;
    // older ones default to not optimizing, so forwarding nothing matches.
    if cmd.build_args.optimize {
        if supports_optimize_flag {
            args.push("--optimize".to_string());
        }
    } else if supports_optimize_false {
        args.push("--optimize=false".to_string());
    }

    args
}

async fn pull_image(docker: &shared::Args, image: &str, print: &Print) -> Result<(), Error> {
    print.infoln(format!("Pulling image {image}"));
    let (stdout, stderr) = if print.quiet {
        (Stdio::null(), Stdio::null())
    } else {
        (Stdio::inherit(), Stdio::inherit())
    };
    let status = docker
        .pull_command(image)
        .stdout(stdout)
        .stderr(stderr)
        .status()
        .await
        .map_err(|e| docker.io_error(e))?;
    if !status.success() {
        return Err(Error::PullImageFailed {
            image: image.to_string(),
        });
    }
    Ok(())
}

/// Run `cmd` in a throwaway `docker run --rm` container (optionally overriding
/// the entrypoint) and return its captured stdout. stderr and the exit status
/// are ignored — every probe treats a missing subcommand or unexpected output as
/// "unsupported".
async fn run_probe(
    image: &str,
    docker: &shared::Args,
    entrypoint: Option<&str>,
    cmd: Vec<String>,
) -> Result<String, Error> {
    let mut command = docker.base_command();
    command.args(["run", "--rm"]);
    if let Some(entrypoint) = entrypoint {
        command.args(["--entrypoint", entrypoint]);
    }
    command.arg(image);
    command.args(&cmd);

    let output = command.output().await.map_err(|e| docker.io_error(e))?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Facts probed from the image before building, gathered in one throwaway
/// container to avoid a round-trip per fact.
struct ImageProbe {
    /// CLI binary on the image's PATH — `stellar` (v21.0.0+) or `soroban`
    /// (older). Used when invoking the CLI by name in the chained multi-build
    /// command; the single-build path uses the image's entrypoint instead.
    bin: String,
    /// Parsed CLI version, or `None` when the image reported no parseable version
    /// (treated as a current image by the caller).
    version: Option<Version>,
    /// The image's default rustup toolchain (e.g.
    /// `1.97.1-aarch64-unknown-linux-gnu`), pinned into `RUSTUP_TOOLCHAIN`.
    /// Guaranteed non-empty — the probe hard-fails when it can't be determined.
    toolchain: String,
}

/// Probe the image once for everything the build needs: the CLI binary name, its
/// version, and the default rustup toolchain. Runs a single `/bin/sh` script
/// (the same `/bin/sh` and `rustup` the multi-build path and toolchain pin
/// already require) that detects the binary, then reports each fact on its own
/// tagged line so the combined stdout can be split apart. Hard-fails when no
/// default toolchain can be determined, rather than building unpinned.
async fn probe_image(image: &str, docker: &shared::Args) -> Result<ImageProbe, Error> {
    // Detect the binary first, then run `$bin version` (version on its first
    // line) and `rustup default` (the toolchain name). Tag each line so we can
    // pick the values back out regardless of any extra output.
    let script = "\
        bin=\"$(command -v stellar >/dev/null 2>&1 && echo stellar || echo soroban)\"\n\
        printf 'BIN:%s\\n' \"$bin\"\n\
        printf 'VERSION:%s\\n' \"$(\"$bin\" version 2>/dev/null | head -n1)\"\n\
        printf 'TOOLCHAIN:%s\\n' \"$(rustup default 2>/dev/null)\"\n";
    let stdout = run_probe(
        image,
        docker,
        Some("/bin/sh"),
        vec!["-c".to_string(), script.to_string()],
    )
    .await?;

    let bin = match probe_value(&stdout, "BIN:") {
        "" => "stellar".to_string(),
        b => b.to_string(),
    };
    let version = parse_cli_version(probe_value(&stdout, "VERSION:"));
    let toolchain = parse_default_toolchain(probe_value(&stdout, "TOOLCHAIN:"))
        .ok_or(Error::ToolchainProbeFailed)?;

    Ok(ImageProbe {
        bin,
        version,
        toolchain,
    })
}

/// Pull the value of a `TAG:value` line out of the combined probe output. Returns
/// an empty string when the tag is absent (the fact couldn't be gathered).
fn probe_value<'a>(stdout: &'a str, tag: &str) -> &'a str {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(tag))
        .map(str::trim)
        .unwrap_or_default()
}

/// Extract the cli version from `version` output. The first line looks like
/// `stellar 27.1.0 (<hash>)` or `soroban-cli 0.1.2 (<hash>)`; later lines carry
/// unrelated numbers (`stellar-xdr 22.1.0`, `soroban-env-interface-version: 23`),
/// so only the first line is considered, taking its first valid-semver token.
fn parse_cli_version(stdout: &str) -> Option<Version> {
    stdout
        .lines()
        .next()?
        .split_whitespace()
        .find_map(|tok| Version::parse(tok).ok())
}

/// Extract the toolchain name from `rustup default` output, which looks like
/// `1.97.1-aarch64-unknown-linux-gnu (default)`. Returns `None` when the output
/// is empty (e.g. the image has no default toolchain or lacks `rustup`).
fn parse_default_toolchain(stdout: &str) -> Option<String> {
    stdout.split_whitespace().next().map(str::to_string)
}

#[allow(clippy::too_many_arguments)]
async fn run_in_container(
    image: &str,
    workspace_root: &Path,
    container_cmds: &[Vec<String>],
    env: &[String],
    docker: &shared::Args,
    run_args: &shared::RunArgs,
    bin: &str,
    print: &Print,
    print_only: bool,
) -> Result<(), Error> {
    let bind = format!("{}:/source", workspace_root.display());
    // The engine prefix for the reproduce line mirrors `base_command`, including
    // `-H <host>` so a copy-paste hits the same daemon the CLI used.
    let prefix = docker.command_prefix();

    // `-e KEY=VALUE` flags, mirrored into the reproduce line below.
    let mut env_flags = String::new();
    for e in env {
        env_flags.push_str(" -e ");
        env_flags.push_str(&shell_escape::escape(e.as_str().into()));
    }

    // On Linux, run as the host uid:gid so wasm the container writes into the
    // bind-mounted `target/` is owned by the invoking user instead of root.
    // Docker Desktop (macOS) and Apple's `container` map ownership to the host
    // user already, so this is Linux-only.
    //
    // This assumes the image keeps CARGO_HOME/RUSTUP_HOME writable by non-root
    // users, which the official rust-based image does. An arbitrary `--image`
    // with root-owned toolchain dirs may fail the build under this uid — a known
    // limitation of running unofficial images.
    let user_flags: Vec<String> = current_user_flags();

    // Run flags for the copy-pasteable reproduce line, matching where they're
    // applied to the spawned command below.
    let mut run_flags = String::new();
    for f in run_args.flags().iter().chain(user_flags.iter()) {
        run_flags.push(' ');
        run_flags.push_str(&shell_escape::escape(f.as_str().into()));
    }

    let (entrypoint, post_image, reproduce) = compose_invocation(
        &prefix,
        &run_flags,
        &bind,
        &env_flags,
        image,
        bin,
        container_cmds,
    );

    // `--print-commands-only`: emit the run command to stdout (so it's
    // pipeable) and stop, without touching the engine.
    if print_only {
        println!("{reproduce}");
        return Ok(());
    }

    print.infoln(format!("Building in {image} (mount {bind})"));
    print.infoln(format!("Running: {reproduce}"));

    // Name the container so it can be stopped if the CLI is interrupted: the
    // daemon owns the container, so the client exiting doesn't stop it. Unique
    // per invocation so concurrent builds don't collide, and kept out of the
    // reproduce line where a fixed name would clash on re-run.
    let container_name = format!(
        "stellar-contract-build-{}-{:08x}",
        std::process::id(),
        rand::random::<u32>()
    );

    let mut command = docker.base_command();
    command.args(["run", "--rm", "--name", &container_name]);
    run_args.apply(&mut command);
    command.args(&user_flags);
    command.args(["-v", &bind, "-w", "/source"]);
    for e in env {
        command.args(["-e", e]);
    }
    if let Some(entrypoint) = entrypoint {
        command.args(["--entrypoint", entrypoint]);
    }
    command.arg(image);
    command.args(&post_image);

    // Stream the build output straight to the terminal (matching a local build);
    // `quiet` discards it.
    let (stdout, stderr) = if print.quiet {
        (Stdio::null(), Stdio::null())
    } else {
        (Stdio::inherit(), Stdio::inherit())
    };
    command.stdout(stdout).stderr(stderr);

    let mut child = command.spawn().map_err(|e| docker.io_error(e))?;

    // Race the build against any catchable termination signal. On a signal, kill
    // the named container (best-effort) so it doesn't outlive the CLI, kill the
    // engine client we spawned, then surface the interruption.
    let status = tokio::select! {
        result = child.wait() => result.map_err(|e| docker.io_error(e))?,
        () = wait_for_termination_signal() => {
            print.warnln("Interrupted; stopping build container");
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

/// `--user <uid>:<gid>` for the current process on Linux, so container-written
/// artifacts on bind mounts are owned by the invoking user rather than root.
/// Empty on every other platform, where the engine's VM maps ownership to the
/// host user already.
#[cfg(target_os = "linux")]
fn current_user_flags() -> Vec<String> {
    let uid = rustix::process::getuid().as_raw();
    let gid = rustix::process::getgid().as_raw();
    vec!["--user".to_string(), format!("{uid}:{gid}")]
}

#[cfg(not(target_os = "linux"))]
fn current_user_flags() -> Vec<String> {
    Vec::new()
}

/// Build the run invocation: the optional entrypoint override, the args after
/// the image, and a copy-pasteable reproduce line (also what
/// `--print-commands-only` emits). One package runs the image's default
/// entrypoint directly; several override the entrypoint to `/bin/sh` and chain
/// the builds (invoking the CLI by `bin` name) so they share one container (and
/// its crates download / compiled deps / `target/`).
fn compose_invocation(
    prefix: &str,
    run_flags: &str,
    bind: &str,
    env_flags: &str,
    image: &str,
    bin: &str,
    container_cmds: &[Vec<String>],
) -> (Option<&'static str>, Vec<String>, String) {
    // The reproduce line is documented as copy-pasteable, so escape the bind
    // mount (which embeds the workspace path) and image ref like every other
    // token; a path with a space or shell metacharacter must still round-trip.
    let bind = shell_escape::escape(bind.into());
    let image = shell_escape::escape(image.into());
    if container_cmds.len() > 1 {
        let chain = compose_shell_command(bin, container_cmds);
        let reproduce = format!(
            "{prefix} run --rm{run_flags} -v {bind} -w /source{env_flags} --entrypoint /bin/sh {image} -c {}",
            shell_escape::escape(chain.clone().into())
        );
        (Some("/bin/sh"), vec!["-c".to_string(), chain], reproduce)
    } else {
        let cmd = container_cmds.first().cloned().unwrap_or_default();
        let reproduce = format!(
            "{prefix} run --rm{run_flags} -v {bind} -w /source{env_flags} {image} {}",
            escape_args(&cmd)
        );
        (None, cmd, reproduce)
    }
}

/// Render the per-package `<bin> contract build …` commands into a single
/// `sh -c` script (`<bin> … && <bin> …`), shell-escaping every token so values
/// with spaces survive. `bin` is the container's CLI binary (`soroban` or
/// `stellar`).
fn compose_shell_command(bin: &str, cmds: &[Vec<String>]) -> String {
    cmds.iter()
        .map(|cmd| {
            std::iter::once(bin)
                .chain(cmd.iter().map(String::as_str))
                .map(|tok| shell_escape::escape(tok.into()).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" && ")
}

/// Shell-escape each token of a single-package command for the reproduce line so
/// a copy-paste round-trips back to the same argv.
fn escape_args(cmd: &[String]) -> String {
    cmd.iter()
        .map(|tok| shell_escape::escape(tok.into()).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resolve once the process receives any catchable signal that would otherwise
/// terminate it, so the caller can stop the build container before exiting.
#[cfg(unix)]
async fn wait_for_termination_signal() {
    use tokio::signal::unix::{signal, SignalKind};

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

/// Await one delivery of an installed signal. When the handler failed to install,
/// never resolves, so it drops out of the `select!` rather than firing spuriously.
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

/// Among candidate artifact paths, return the one that exists and was modified
/// most recently. Probing by existence alone can return a stale wasm left by an
/// earlier build into a different target-triple dir; the freshest file is the
/// one the current build just wrote. Returns `None` when none exist. An
/// unreadable mtime is treated as the epoch, so such a file is only chosen when
/// it's the sole candidate.
fn newest_existing_artifact(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .filter(|p| p.exists())
        .max_by_key(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        })
        .cloned()
}

/// Collect the built wasm from the mounted `target/`. Because the working tree
/// was bind-mounted, the container writes artifacts straight to the host under
/// `<workspace>/target/<triple>/<profile>/`. The container's rust toolchain
/// decides the target triple, so both known triples are probed. Copies to
/// `--out-dir` when set.
fn collect_built_contracts(
    cmd: &Cmd,
    md: &cargo_metadata::Metadata,
    workspace_root: &Path,
) -> Result<Vec<BuiltContract>, super::Error> {
    let target_root = workspace_root.join("target");

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

        let file = format!("{}.wasm", p.name.replace('-', "_"));
        // The container may build for either wasm target depending on its rust
        // version, so probe both triple dirs. Pick the *freshest* rather than the
        // first that exists: an earlier build into the other triple can leave a
        // stale wasm behind, and selecting by existence alone would return it.
        // Fall back to the current host default for the reported path when the
        // build produced nothing.
        let candidates: Vec<PathBuf> = [WASM_TARGET, WASM_TARGET_OLD]
            .iter()
            .map(|triple| target_root.join(triple).join(&cmd.profile).join(&file))
            .collect();
        let src = newest_existing_artifact(&candidates).unwrap_or_else(|| {
            let triple = get_wasm_target().unwrap_or_else(|_| WASM_TARGET.to_string());
            target_root.join(triple).join(&cmd.profile).join(&file)
        });

        let path = if let Some(out_dir) = &cmd.out_dir {
            std::fs::create_dir_all(out_dir).map_err(super::Error::CreatingOutDir)?;
            let dest = out_dir.join(&file);
            if src.exists() {
                std::fs::copy(&src, &dest).map_err(super::Error::CopyingWasmFile)?;
            }
            dest
        } else {
            src
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
    use crate::commands::contract::build::BuildArgs;

    fn ws() -> &'static Path {
        Path::new("/tmp/ws")
    }

    #[test]
    fn forwarded_build_args_defaults() {
        let cmd = Cmd::default();
        let args = forwarded_build_args(&cmd, ws(), None, true, true, true);
        assert_eq!(args[..2], ["contract".to_string(), "build".to_string()]);
        // Default optimize=true → bare `--optimize`; no `--locked` unless asked.
        assert!(args.contains(&"--optimize".to_string()));
        assert!(!args.iter().any(|a| a == "--locked"));
        assert!(!args.iter().any(|a| a.starts_with("--package")));
    }

    #[test]
    fn forwarded_build_args_locked_and_package() {
        let cmd = Cmd {
            locked: true,
            ..Cmd::default()
        };
        let args = forwarded_build_args(&cmd, ws(), Some("contract-a"), true, true, true);
        assert!(args.contains(&"--locked".to_string()));
        assert!(args.contains(&"--package=contract-a".to_string()));
    }

    #[test]
    fn forwarded_build_args_drops_locked_when_unsupported() {
        // User asked for --locked but the image's cli doesn't accept it.
        let cmd = Cmd {
            locked: true,
            ..Cmd::default()
        };
        let args = forwarded_build_args(&cmd, ws(), None, false, true, true);
        assert!(!args.iter().any(|a| a == "--locked"));
    }

    #[test]
    fn forwarded_build_args_omits_optimize_when_flag_unsupported() {
        // Image older than v23.2.0 has no `--optimize` flag; forward nothing even
        // though optimize defaults to true.
        let cmd = Cmd::default();
        assert!(cmd.build_args.optimize);
        let args = forwarded_build_args(&cmd, ws(), None, true, false, false);
        assert!(!args.iter().any(|a| a.starts_with("--optimize")));
    }

    #[test]
    fn forwarded_build_args_features_meta_and_profile() {
        let cmd = Cmd {
            profile: "dev".to_string(),
            features: Some("a,b".to_string()),
            all_features: true,
            no_default_features: true,
            build_args: BuildArgs {
                meta: vec![
                    ("home_domain".to_string(), "example.com".to_string()),
                    ("author".to_string(), "alice".to_string()),
                ],
                optimize: false,
            },
            ..Cmd::default()
        };
        let args = forwarded_build_args(&cmd, ws(), None, true, true, true);
        assert!(args.contains(&"--profile=dev".to_string()));
        assert!(args.contains(&"--features=a,b".to_string()));
        assert!(args.contains(&"--all-features".to_string()));
        assert!(args.contains(&"--no-default-features".to_string()));
        assert!(args.contains(&"--meta=home_domain=example.com".to_string()));
        assert!(args.contains(&"--meta=author=alice".to_string()));
        assert!(args.contains(&"--optimize=false".to_string()));
    }

    #[test]
    fn forwarded_build_args_optimize_false_old_image_forwards_nothing() {
        // Old image defaults to not optimizing and rejects `--optimize=false`,
        // so nothing about optimize is forwarded.
        let cmd = Cmd {
            build_args: BuildArgs {
                optimize: false,
                ..BuildArgs::default()
            },
            ..Cmd::default()
        };
        let args = forwarded_build_args(&cmd, ws(), None, true, true, false);
        assert!(!args.iter().any(|a| a.starts_with("--optimize")));
    }

    #[test]
    fn forwarded_build_args_relativizes_manifest_path() {
        let cmd = Cmd {
            manifest_path: Some(PathBuf::from("/tmp/ws/contracts/add/Cargo.toml")),
            ..Cmd::default()
        };
        let args = forwarded_build_args(&cmd, ws(), None, true, true, true);
        assert!(args.contains(&"--manifest-path=contracts/add/Cargo.toml".to_string()));
    }

    #[test]
    fn compose_shell_command_chains_and_escapes() {
        let a = vec![
            "contract".to_string(),
            "build".to_string(),
            "--package=another".to_string(),
            "--meta=note=added on build".to_string(),
        ];
        let b = vec![
            "contract".to_string(),
            "build".to_string(),
            "--package=hello-world".to_string(),
        ];
        let s = compose_shell_command("stellar", &[a.clone(), b.clone()]);
        assert!(s.contains("stellar contract build --package=another"));
        assert!(s.contains("&&"));
        assert!(s.contains("stellar contract build --package=hello-world"));
        // A value with a space must be quoted so it stays one token.
        assert!(
            s.contains("'--meta=note=added on build'")
                || s.contains("\"--meta=note=added on build\""),
            "expected the spaced token to be quoted, got: {s}"
        );

        // An older image's binary (`soroban`) is used verbatim in the chain.
        let s = compose_shell_command("soroban", &[a, b]);
        assert!(s.contains("soroban contract build --package=another"));
        assert!(s.contains("soroban contract build --package=hello-world"));
        assert!(!s.contains("stellar"));
    }

    #[test]
    fn compose_invocation_single_package_uses_default_entrypoint() {
        let cmds = vec![vec![
            "contract".to_string(),
            "build".to_string(),
            "--meta=field=value".to_string(),
            "--optimize".to_string(),
        ]];
        let (entrypoint, post_image, reproduce) = compose_invocation(
            "docker",
            "",
            "/ws:/source",
            " -e CARGO_TARGET_DIR=/source/target",
            "docker.io/stellar/stellar-cli:latest",
            "stellar",
            &cmds,
        );
        assert!(entrypoint.is_none());
        assert_eq!(post_image, cmds[0]);
        // The bind mount and image ref are shell-escaped (single-quoted here
        // because of the `:`), so the line copy-pastes back to the same argv.
        assert_eq!(
            reproduce,
            "docker run --rm -v '/ws:/source' -w /source \
             -e CARGO_TARGET_DIR=/source/target \
             'docker.io/stellar/stellar-cli:latest' \
             contract build --meta=field=value --optimize"
        );
    }

    #[test]
    fn compose_invocation_escapes_spaced_bind_and_image() {
        // A workspace path with a space must stay one token in the copy-pasteable
        // reproduce line, as must a metacharacter-laden image ref.
        let cmds = vec![vec!["contract".to_string(), "build".to_string()]];
        let (_entrypoint, _post_image, reproduce) = compose_invocation(
            "docker",
            "",
            "/Users/me/My Project/ws:/source",
            "",
            "my registry/img:tag",
            "stellar",
            &cmds,
        );
        // The whole `-v` value and the image ref round-trip as single tokens.
        let tokens = shlex::split(&reproduce).expect("reproduce line must be valid shell");
        assert!(tokens.contains(&"/Users/me/My Project/ws:/source".to_string()));
        assert!(tokens.contains(&"my registry/img:tag".to_string()));
    }

    #[test]
    fn compose_invocation_includes_engine_prefix_verbatim() {
        // The prefix (which may carry `-H <host>`) is rendered before `run`, so a
        // copy-paste hits the same daemon the CLI used.
        let cmds = vec![vec!["contract".to_string(), "build".to_string()]];
        let (_entrypoint, _post_image, reproduce) = compose_invocation(
            "docker -H ssh://host",
            "",
            "/ws:/source",
            "",
            "img:tag",
            "stellar",
            &cmds,
        );
        assert!(
            reproduce.starts_with(
                "docker -H ssh://host run --rm -v '/ws:/source' -w /source 'img:tag' contract build"
            ),
            "got: {reproduce}"
        );
    }

    #[test]
    fn compose_invocation_multi_package_chains_through_shell() {
        let cmds = vec![
            vec![
                "contract".to_string(),
                "build".to_string(),
                "--package=a".to_string(),
            ],
            vec![
                "contract".to_string(),
                "build".to_string(),
                "--package=b".to_string(),
            ],
        ];
        let (entrypoint, post_image, reproduce) = compose_invocation(
            "container",
            " --cpus 2",
            "/ws:/source",
            "",
            "img:tag",
            "stellar",
            &cmds,
        );
        assert_eq!(entrypoint, Some("/bin/sh"));
        assert_eq!(post_image[0], "-c");
        assert_eq!(
            post_image[1],
            "stellar contract build --package=a && stellar contract build --package=b"
        );
        assert!(reproduce
            .starts_with("container run --rm --cpus 2 -v '/ws:/source' -w /source --entrypoint /bin/sh 'img:tag' -c "));
        // The chained script is passed as one shell-quoted argument.
        assert!(reproduce.contains(
            "'stellar contract build --package=a && stellar contract build --package=b'"
        ));

        // A pre-21.0.0 image's `soroban` binary flows through to the chain.
        let (_entrypoint, post_image, reproduce) = compose_invocation(
            "container",
            "",
            "/ws:/source",
            "",
            "img:tag",
            "soroban",
            &cmds,
        );
        assert_eq!(
            post_image[1],
            "soroban contract build --package=a && soroban contract build --package=b"
        );
        assert!(reproduce.contains(
            "'soroban contract build --package=a && soroban contract build --package=b'"
        ));
    }

    #[test]
    fn parse_cli_version_reads_first_line_only() {
        // Old `soroban` binary: must take 0.1.2, not the `23` on the next line.
        assert_eq!(
            parse_cli_version(
                "soroban-cli 0.1.2 (70110a1eb3e3af0bee4ac93d005eb2614e9c8e85)\n\
                 soroban-env-interface-version: 23\n"
            ),
            Some(Version::parse("0.1.2").unwrap())
        );
        // Current `stellar` binary: must take 27.1.0, not the stellar-xdr 22.1.0.
        assert_eq!(
            parse_cli_version(
                "stellar 27.1.0 (abc123)\n\
                 stellar-xdr 22.1.0 (def456)\n\
                 xdr curr (ghi789)\n"
            ),
            Some(Version::parse("27.1.0").unwrap())
        );
        // No trailing git hash.
        assert_eq!(
            parse_cli_version("stellar 26.1.0\n"),
            Some(Version::parse("26.1.0").unwrap())
        );
        assert_eq!(parse_cli_version(""), None);
        assert_eq!(parse_cli_version("not a version\n"), None);
    }

    #[test]
    fn probe_value_splits_tagged_combined_output() {
        let stdout = "BIN:stellar\n\
                      VERSION:stellar 27.1.0 (abc123)\n\
                      TOOLCHAIN:1.97.1-aarch64-unknown-linux-gnu (default)\n";
        assert_eq!(probe_value(stdout, "BIN:"), "stellar");
        assert_eq!(probe_value(stdout, "VERSION:"), "stellar 27.1.0 (abc123)");
        assert_eq!(
            probe_value(stdout, "TOOLCHAIN:"),
            "1.97.1-aarch64-unknown-linux-gnu (default)"
        );
        // The tagged values feed the same parsers used on standalone output.
        assert_eq!(
            parse_cli_version(probe_value(stdout, "VERSION:")),
            Some(Version::parse("27.1.0").unwrap())
        );
        assert_eq!(
            parse_default_toolchain(probe_value(stdout, "TOOLCHAIN:")).as_deref(),
            Some("1.97.1-aarch64-unknown-linux-gnu")
        );
        // A missing tag (fact not gathered) yields an empty value.
        assert_eq!(probe_value("BIN:soroban\n", "TOOLCHAIN:"), "");
    }

    #[test]
    fn parse_default_toolchain_extracts_name() {
        assert_eq!(
            parse_default_toolchain("1.97.1-aarch64-unknown-linux-gnu (default)\n").as_deref(),
            Some("1.97.1-aarch64-unknown-linux-gnu")
        );
        assert_eq!(
            parse_default_toolchain("stable-x86_64-unknown-linux-gnu (default)").as_deref(),
            Some("stable-x86_64-unknown-linux-gnu")
        );
        assert_eq!(parse_default_toolchain("").as_deref(), None);
        assert_eq!(parse_default_toolchain("   \n").as_deref(), None);
    }

    #[test]
    fn newest_existing_artifact_prefers_freshest_not_first() {
        use std::time::{Duration, SystemTime};
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.wasm");
        let new = dir.path().join("new.wasm");
        std::fs::write(&old, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        // Pin mtimes so the ordering is unambiguous regardless of filesystem
        // timestamp resolution.
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(base)
            .unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&new)
            .unwrap()
            .set_modified(base + Duration::from_mins(1))
            .unwrap();

        // `old` is listed first, but the fresher `new` must win — selection is by
        // mtime, not list position (the staleness bug this guards against).
        assert_eq!(
            newest_existing_artifact(&[old.clone(), new.clone()]),
            Some(new)
        );
        // A non-existent candidate is skipped; the one real file is returned.
        let missing = dir.path().join("missing.wasm");
        assert_eq!(
            newest_existing_artifact(&[missing.clone(), old.clone()]),
            Some(old)
        );
        // Nothing exists → None (caller falls back to the host-default path).
        assert_eq!(newest_existing_artifact(&[missing]), None);
    }

    #[test]
    fn escape_args_round_trips_spaced_tokens() {
        let cmd = vec![
            "contract".to_string(),
            "build".to_string(),
            "--meta=note=added on build".to_string(),
        ];
        let s = escape_args(&cmd);
        let tokens = shlex::split(&s).expect("reproduce args must be valid shell");
        assert_eq!(
            tokens,
            vec!["contract", "build", "--meta=note=added on build"]
        );
    }
}
