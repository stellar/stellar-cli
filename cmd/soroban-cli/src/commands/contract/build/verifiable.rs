//! Verifiable (SEP-58 reproducible) contract builds.
//!
//! Triggered by `stellar contract build --verifiable`. Unlike the plain
//! `--image` container build, this snapshots the working tree into a
//! byte-reproducible source archive, hashes it (`source_sha256`), extracts it
//! into a permission-hardened tempdir, and builds *that* in a digest-pinned
//! image — recording SEP-58 provenance meta (`bldimg`, `source_uri`,
//! `source_sha256`, `bldopt`) into the wasm so a third party can reproduce the
//! exact bytes.
//!
//! The container execution machinery (image probe, `run_in_container`,
//! reproduce lines, artifact collection) is shared with
//! [`super::container`]; this module adds the archive and the SEP-58 metadata
//! on top. The build image is the user-supplied, digest-pinned `--image`.

use std::path::{Path, PathBuf};

use regex::Regex;
use semver::Version;
use sha2::{Digest, Sha256};
use soroban_spec_tools::sanitize;

use crate::{
    commands::{
        container::shared::{self, Error as ConnectionError},
        global,
    },
    config::{
        data,
        locator::{enforce_hardened_tree, write_hardened_file},
    },
    print::Print,
};

use super::{container, source_archive, BuiltContract, Cmd};

const RESERVED_META_KEYS: &[&str] = &["bldimg", "source_uri", "source_sha256", "bldopt"];

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    DockerConnection(#[from] ConnectionError),

    #[error("--image value {value:?} does not match the SEP-58 bldimg format `<registry-host>/<repo>@sha256:<64-hex>`. Examples: docker.io/stellar/stellar-cli@sha256:<64-hex>, localhost:5000/foo@sha256:<64-hex>. Tag-only refs and implicit Docker-Hub short refs are not accepted.")]
    BldimgFormat { value: String },

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

    #[error("SEP-58 metadata must be ASCII, but {field} contains non-ASCII characters: {value}. Use an ASCII value (e.g. a punycode/percent-encoded URI).")]
    NonAsciiMeta { field: String, value: String },

    #[error(transparent)]
    Data(#[from] data::Error),
}

pub async fn run(
    cmd: &Cmd,
    global_args: &global::Args,
    print: &Print,
) -> Result<Vec<BuiltContract>, super::Error> {
    let _ = global_args;

    // Stage 1: pure validation, no I/O.
    for (k, _) in &cmd.build_args.meta {
        if RESERVED_META_KEYS.iter().any(|r| r == k) {
            return Err(Error::ReservedMetaKey { key: k.clone() }.into());
        }
    }
    if let Some(img) = &cmd.image {
        validate_image(img)?;
    }

    // Stage 2: local filesystem + git, no network.
    validate_source_formats(cmd)?;

    // The source root is the current working directory: it's archived,
    // bind-mounted into the container, and the `--manifest-path` bldopt is
    // relativized against it. Run from the project/workspace root you want built.
    let source_root = source_archive::resolve_source_root();

    // The archive is the working tree, so refuse a dirty repo: a verifiable build
    // should be deliberate, off a committed state, not whatever happens to be on
    // disk. Skipped when the source root isn't a git repo.
    source_archive::ensure_clean_tree(&source_root, print).map_err(Error::from)?;

    // Build the source archive, record its hash, and build from the *extracted*
    // archive (in a hardened tempdir) so the wasm is produced from exactly the
    // bytes that were hashed.
    let resolved = {
        let a = resolve_archive(cmd, &source_root, print)?;
        // The extracted `source/` dir mirrors `source_root` exactly and is both
        // the container mount and the tree the build writes `target/` into.
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

    // Stage 3: the container engine.
    let docker = cmd.container_args.clone();
    docker.warn_if_host_ignored(print);
    let image_ref = resolve_image(cmd, &docker, print).await?;

    // Probe the pinned image once for its cli binary, version, and default
    // toolchain (shared with the plain container build), then gate flags on the
    // reported version.
    let probe = container::probe_image(&image_ref, &docker).await?;
    let cli_version = probe.version.clone();
    let at_least = |min: &str| {
        cli_version
            .as_ref()
            .is_none_or(|v| *v >= Version::parse(min).unwrap())
    };
    let supports_locked = at_least(container::LOCKED_MIN);
    let supports_optimize_flag = at_least(container::OPTIMIZE_FLAG_MIN);
    let supports_optimize_false = at_least(container::OPTIMIZE_NEW_SYNTAX_MIN);

    // `--locked` is implied by `--verifiable` (a reproducible build should pin
    // the lockfile), but it was only added to `contract build` in cli 25.2.0.
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

    // Resolve host `cargo metadata` once and reuse it for package selection and
    // artifact collection, mirroring the plain container build.
    let md = container::metadata(cmd).map_err(container::Error::Metadata)?;

    // Build once per package, each with its own `--package` forwarded and
    // recorded as a `bldopt`, so every wasm is independently reproducible.
    let packages = container::resolve_packages(cmd, &md);
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
            // Verifiable implies `--locked` (when supported) and records every
            // build-affecting flag as a `bldopt`.
            let (mut args, bldopts) = container::forwarded_build_args(
                cmd,
                &source_root,
                *target,
                supports_locked,
                supports_optimize_flag,
                supports_optimize_false,
                true,
            );
            args.extend(build_metadata_args(&image_ref, &source_ids, &bldopts));
            args
        })
        .collect();

    // Pin the target dir to a known location under the mount, and the image's
    // own default toolchain so a `rust-toolchain.toml` in the source can't
    // redirect the build to a toolchain rustup would then try to install.
    let mut env = vec!["CARGO_TARGET_DIR=/source/target".to_string()];
    print.infoln(format!("Using Rust toolchain {}", probe.toolchain));
    env.push(format!("RUSTUP_TOOLCHAIN={}", probe.toolchain));

    container::run_in_container(
        &image_ref,
        &resolved.mount_root,
        &container_cmds,
        &env,
        &docker,
        &cmd.run_args,
        &probe.bin,
        "stellar-verifiable-build",
        print,
        cmd.print_commands_only,
    )
    .await?;

    // Nothing was built when only printing the command.
    if cmd.print_commands_only {
        return Ok(Vec::new());
    }

    container::collect_built_contracts(cmd, &md, &source_root, resolved.extracted_root.as_deref())
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

/// Source-identification fields recorded as SEP-58 meta. `source_sha256` is
/// always `Some` by the time these are built in `run()` (computed from the
/// generated archive). `source_uri` is `Some` only when the user passed
/// `--source-uri`.
#[derive(Debug, Default, Clone)]
struct SourceIds {
    source_uri: Option<String>,
    source_sha256: Option<String>,
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
        // SEP-58 metadata is ASCII, but the URI regex's `\S` is Unicode-aware, so
        // guard explicitly before the format check.
        if !uri.is_ascii() {
            return Err(Error::NonAsciiMeta {
                field: "source_uri".to_string(),
                value: sanitize(uri),
            });
        }
        if !source_uri_regex().is_match(uri) {
            return Err(Error::SourceUriFormat { value: uri.clone() });
        }
    }
    Ok(())
}

/// Validate the SEP-58 `bldimg` (`--image`): it must be ASCII (the format regex
/// is Unicode-aware, so guard explicitly) and match the digest-pinned format.
/// The offending value is sanitized for display.
fn validate_image(image: &str) -> Result<(), Error> {
    if !image.is_ascii() {
        return Err(Error::NonAsciiMeta {
            field: "bldimg".to_string(),
            value: sanitize(image),
        });
    }
    if !bldimg_regex().is_match(image) {
        return Err(Error::BldimgFormat {
            value: image.to_string(),
        });
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
    let bytes = source_archive::build_source_archive(source_root, print, true, None)?;
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

    // Content-addressed name under the managed archives dir. The archive is the
    // whole working tree, so it can hold private source or an unignored `.env`;
    // write it `0600` (never the umask default `0644`) so it isn't world-readable.
    let out_path = data::archives_dir()?.join(format!("{computed}.tar.gz"));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| source_archive::Error::ArchiveWrite {
            path: out_path.clone(),
            source,
        })?;
    }
    write_hardened_file(&out_path, &bytes).map_err(|source| {
        source_archive::Error::ArchiveWrite {
            path: out_path.clone(),
            source,
        }
    })?;
    print.infoln(format!(
        "Wrote source archive {} (source_sha256 {computed})",
        out_path.display()
    ));

    // Extract and harden, then build from the extracted copy so the wasm is
    // produced from exactly the archived bytes.
    //
    // Extract under the data dir, NOT the OS temp dir: on macOS `$TMPDIR` lives
    // under /var/folders, which container VMs (Docker Desktop, Colima, …) don't
    // share by default, so a bind mount of it would be empty inside the
    // container. The data dir lives under the user's home, which is shared.
    let base = data::data_local_dir()?;
    std::fs::create_dir_all(&base).map_err(|source| source_archive::Error::ArchiveWrite {
        path: base.clone(),
        source,
    })?;
    let tmp = tempfile::Builder::new()
        .prefix("verifiable-src-")
        .tempdir_in(&base)
        .map_err(source_archive::Error::ArchiveExtract)?;
    source_archive::unpack_targz(&bytes, tmp.path())?;
    enforce_hardened_tree(tmp.path()).map_err(source_archive::Error::ArchiveExtract)?;

    let extracted_root = tmp.path().to_path_buf();
    Ok(ArchiveResult {
        source_sha256: computed,
        extracted_root,
        tmp,
    })
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

/// Emit the SEP-58 `--meta` pairs recorded into the wasm: `bldimg` (the pinned
/// image digest) first, then `source_uri`/`source_sha256` when present, then one
/// `bldopt` per recorded build option. The bldopts already arrive as valid shell
/// (escaped at the source in `forwarded_build_args`), so a verifier reconstructs
/// the build by joining the recorded values and running them through a shell.
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

/// The image to build in and record as `bldimg`: the user-supplied,
/// digest-pinned `--image` (required by clap and already format-validated in
/// `run`). It's a content-addressed ref, so it names the exact bytes as-is;
/// `--pull` refreshes it up front, otherwise a missing image is fetched by the
/// run itself, matching the plain container build.
async fn resolve_image(cmd: &Cmd, docker: &shared::Args, print: &Print) -> Result<String, Error> {
    let image = cmd
        .image
        .clone()
        .expect("--image is required with --verifiable (enforced by clap)");
    if cmd.pull {
        docker.pull_image(&image, print).await?;
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // SEP-58 metadata must be ASCII; the URI regex's `\S` is Unicode-aware, so a
    // non-ASCII (but otherwise well-formed) URI must still be rejected.
    #[test]
    fn validate_image_checks_ascii_then_format() {
        // Non-ASCII registry/repo → rejected before the format check.
        let err =
            validate_image(&format!("localhost:5000/café@sha256:{}", "0".repeat(64))).unwrap_err();
        assert!(matches!(err, Error::NonAsciiMeta { .. }), "got {err:?}");
        // ASCII but tag-only → format error.
        let err = validate_image("docker.io/stellar/stellar-cli:latest").unwrap_err();
        assert!(matches!(err, Error::BldimgFormat { .. }), "got {err:?}");
        // ASCII, digest-pinned → ok.
        validate_image(&format!(
            "docker.io/stellar/stellar-cli@sha256:{}",
            "a".repeat(64)
        ))
        .unwrap();
    }

    #[test]
    fn validate_source_formats_rejects_non_ascii_uri() {
        let cmd = Cmd {
            source_uri: Some("https://例.example/src.tar.gz".to_string()),
            source_sha256: Some("a".repeat(64)),
            ..Cmd::default()
        };
        let err = validate_source_formats(&cmd).unwrap_err();
        assert!(matches!(err, Error::NonAsciiMeta { .. }), "got {err:?}");
    }

    #[test]
    fn validate_source_formats_accepts_valid_and_absent() {
        // Both absent is fine here — requiredness is enforced by clap/run().
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
    fn reserved_meta_keys_list() {
        for key in ["bldimg", "source_uri", "source_sha256", "bldopt"] {
            assert!(RESERVED_META_KEYS.contains(&key));
        }
    }
}
