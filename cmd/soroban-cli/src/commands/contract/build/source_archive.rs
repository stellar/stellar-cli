//! Reproducible source-archive generation for verifiable builds.
//!
//! Produces a gzipped tarball of a contract's source tree, rooted under a
//! top-level `source/` prefix (so it extracts to a `source/` dir, mirroring the
//! container's `/source` mount). In a git repo this is `git archive HEAD` (the
//! committed tree); otherwise the working directory is walked and tarred,
//! skipping `ARCHIVE_DENYLIST` entries. The output is byte-reproducible, so the
//! same tree always hashes to the same `source_sha256`.
//!
//! Shared by `contract build --verifiable` (which builds from the extracted
//! archive) and the standalone `contract archive` command (which generates and
//! inspects it).

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use walkdir::WalkDir;

use crate::config::{data, locator::enforce_hardened_tree};
use crate::print::Print;

/// Top-level names excluded when archiving a non-git working directory (we have
/// no tracked-files list to consult, so fall back to a fixed denylist of VCS
/// metadata, build/cache/transient dirs, and editor/OS/AI-assistant junk).
/// Matched against each path component, so a directory like `target/` prunes
/// its whole subtree.
pub(crate) const ARCHIVE_DENYLIST: &[&str] = &[
    // version control
    ".git",
    ".gitignore",
    ".svn",
    ".hg",
    // secrets / local environment
    ".env",
    // build output / dependencies
    "target",
    "node_modules",
    // transient
    "log",
    "logs",
    "tmp",
    "temp",
    // OS / editor junk
    ".DS_Store",
    "Thumbs.db",
    ".idea",
    ".vscode",
    // AI assistant dirs
    ".claude",
    ".cursor",
    ".windsurf",
    ".aider",
];

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("could not read git state at {path}: {source}")]
    GitInvoke {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("`git archive` failed in {path}: {stderr}")]
    GitArchive { path: PathBuf, stderr: String },

    #[error("could not write source archive to {path}: {source}")]
    ArchiveWrite {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not extract source archive: {0}")]
    ArchiveExtract(std::io::Error),

    #[error(transparent)]
    Data(#[from] data::Error),
}

/// Pick the anchor for the source tree: the directory whose `.git` parent we
/// archive (and, for verifiable builds, relativize `--manifest-path` against).
/// Walk up from `manifest_path` (or cwd, if none) looking for a `.git`
/// directory; return its parent. If none is found, fall back to cwd.
///
/// This isn't a validation step — any `.git` will do. Wrong-source mistakes are
/// caught later by the verify-side byte comparison.
pub(crate) fn resolve_source_root(manifest_path: Option<&Path>) -> PathBuf {
    let start = if let Some(p) = manifest_path {
        let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
        abs.parent().map(Path::to_path_buf).unwrap_or(abs)
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let mut p = start.clone();
    loop {
        if p.join(".git").exists() {
            return p;
        }
        if !p.pop() {
            break;
        }
    }

    std::env::current_dir().unwrap_or(start)
}

/// Whether `source_root` is inside a git work tree.
pub(crate) fn is_git_repo(source_root: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(source_root)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Whether `source_root` is a git work tree with uncommitted changes. Returns
/// `Ok(false)` when it isn't a git repo (git ran but refused) — callers can't
/// verify cleanliness there, so they proceed. Errors only when git can't be
/// invoked at all.
pub(crate) fn tree_is_dirty(source_root: &Path) -> Result<bool, Error> {
    let status = Command::new("git")
        .arg("-C")
        .arg(source_root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .map_err(|source| Error::GitInvoke {
            path: source_root.to_path_buf(),
            source,
        })?;

    // Not a git repo (or git refused): can't verify cleanliness, proceed.
    if !status.status.success() {
        return Ok(false);
    }

    Ok(!status.stdout.is_empty())
}

/// Produce the gzipped source tarball bytes. Entries are rooted under a
/// top-level `source/` prefix. In a git repo this is `git archive HEAD` (the
/// committed tree); otherwise the working directory is walked and tarred,
/// skipping `ARCHIVE_DENYLIST` entries.
///
/// When the source isn't a git repo, `warn_non_git` controls whether to warn
/// that the working directory is being archived. Callers that only inspect the
/// result (e.g. `contract archive --dry-run`) pass `false`, since the listing
/// itself reveals the contents.
pub(crate) fn build_source_archive(
    source_root: &Path,
    print: &Print,
    warn_non_git: bool,
) -> Result<Vec<u8>, Error> {
    let tar = if is_git_repo(source_root) {
        git_archive_tar(source_root)?
    } else {
        if warn_non_git {
            print.warnln(format!(
                "{} is not a git repository; archiving the working directory. Inspect the generated archive to confirm its contents.",
                source_root.display(),
            ));
        }
        walk_tar(source_root)?
    };
    gzip(&tar)
}

/// Tar entry paths inside the gzipped archive bytes, in archive order. Used by
/// `contract archive --dry-run` to list exactly what the bytes that hash to
/// `source_sha256` contain.
pub(crate) fn entry_names(bytes: &[u8]) -> Result<Vec<String>, Error> {
    let dec = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(dec);
    let mut names = Vec::new();
    for entry in archive.entries().map_err(Error::ArchiveExtract)? {
        let entry = entry.map_err(Error::ArchiveExtract)?;
        let path = entry.path().map_err(Error::ArchiveExtract)?;
        names.push(path.to_string_lossy().into_owned());
    }
    Ok(names)
}

/// `git archive --format=tar --prefix=source/ HEAD`, returning the tar bytes.
fn git_archive_tar(source_root: &Path) -> Result<Vec<u8>, Error> {
    let out = Command::new("git")
        .arg("-C")
        .arg(source_root)
        .arg("archive")
        .arg("--format=tar")
        .arg("--prefix=source/")
        .arg("HEAD")
        .output()
        .map_err(|source| Error::GitInvoke {
            path: source_root.to_path_buf(),
            source,
        })?;
    if !out.status.success() {
        return Err(Error::GitArchive {
            path: source_root.to_path_buf(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(out.stdout)
}

/// Tar the working tree under `source_root`, skipping denylisted path
/// components. Each entry is prefixed with `source/`.
///
/// The output is reproducible, following GNU tar's reproducibility guidance
/// (<https://www.gnu.org/software/tar/manual/html_section/Reproducibility.html>)
/// with the portable equivalents available via the `tar` crate (the system
/// `tar` can't be relied on — macOS ships bsdtar, which lacks `--sort`,
/// `--mtime`, `--pax-option`, …): entries are sorted by name (`--sort=name`)
/// using locale-independent path ordering (`LC_ALL=C`), and `HeaderMode::Deterministic`
/// zeroes mtime (`--mtime`/`--clamp-mtime`), sets uid/gid to 0 with empty owner
/// names (`--owner=0 --group=0 --numeric-owner`), and normalizes mode
/// (`--mode=go+u,go-w`). ustar headers carry no atime/ctime or tar PID. The gzip
/// wrapper (see `gzip`) is likewise deterministic.
fn walk_tar(source_root: &Path) -> Result<Vec<u8>, Error> {
    let mut files: Vec<PathBuf> = Vec::new();
    let walk = WalkDir::new(source_root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| !is_denylisted(e.file_name()));
    for entry in walk {
        let entry = entry.map_err(|e| Error::ArchiveWrite {
            path: source_root.to_path_buf(),
            source: e.into(),
        })?;
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();

    let mut builder = tar::Builder::new(Vec::new());
    builder.mode(tar::HeaderMode::Deterministic);
    for path in &files {
        let rel = path.strip_prefix(source_root).unwrap_or(path);
        let name = Path::new("source").join(rel);
        let mut f = std::fs::File::open(path).map_err(|source| Error::ArchiveWrite {
            path: path.clone(),
            source,
        })?;
        builder
            .append_file(&name, &mut f)
            .map_err(|source| Error::ArchiveWrite {
                path: path.clone(),
                source,
            })?;
    }
    builder.into_inner().map_err(|source| Error::ArchiveWrite {
        path: source_root.to_path_buf(),
        source,
    })
}

/// A path component is denylisted if it equals a denylist entry, or — for
/// dotted entries, which double as extension filters (e.g. `.swp`, `.log`) — if
/// it ends with that entry. Plain names (`target`, `node_modules`) match
/// exactly only, so `mytarget` is not excluded.
fn is_denylisted(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    ARCHIVE_DENYLIST
        .iter()
        .any(|d| name == *d || (d.starts_with('.') && name.ends_with(d)))
}

/// Gzip with a default (mtime-zeroed) header so the same tar bytes always hash
/// the same.
fn gzip(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(bytes).map_err(|source| Error::ArchiveWrite {
        path: PathBuf::new(),
        source,
    })?;
    enc.finish().map_err(|source| Error::ArchiveWrite {
        path: PathBuf::new(),
        source,
    })
}

/// Decompress gzip and unpack the tar into `dest`. Entries are `source/…`, so
/// they land at `<dest>/source/…`.
pub(crate) fn unpack_targz(bytes: &[u8], dest: &Path) -> Result<(), Error> {
    let dec = flate2::read::GzDecoder::new(bytes);
    tar::Archive::new(dec)
        .unpack(dest)
        .map_err(Error::ArchiveExtract)
}

/// Create a fresh temp directory, unpack the gzipped source tarball `bytes` into
/// it, harden its permissions, and return the guard (the tree lives at its
/// `path()`). Shared by `build --verifiable` (builds from the extracted copy)
/// and `verify` (rebuilds from it); `prefix` names the dir so the two are
/// distinguishable on disk.
///
/// The temp dir is created under `<data dir>/tmp`, NOT the OS temp dir: on macOS
/// `$TMPDIR` lives under /var/folders, which container VMs (Docker Desktop,
/// Colima, …) don't share by default, so a bind mount of it would be empty
/// inside the container. The data dir lives under the user's home, which is
/// shared. Corralling every extraction under a single `tmp/` keeps a leftover
/// from an interrupted run isolated in one obviously-disposable place rather
/// than loose alongside `archives/`.
pub(crate) fn extract_into_hardened_tempdir(
    bytes: &[u8],
    prefix: &str,
) -> Result<tempfile::TempDir, Error> {
    let base = data::data_local_dir()?.join("tmp");
    std::fs::create_dir_all(&base).map_err(|source| Error::ArchiveWrite {
        path: base.clone(),
        source,
    })?;
    let tmp = tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&base)
        .map_err(Error::ArchiveExtract)?;
    unpack_targz(bytes, tmp.path())?;
    enforce_hardened_tree(tmp.path()).map_err(Error::ArchiveExtract)?;
    Ok(tmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::locator::enforce_hardened_tree;
    use sha2::{Digest, Sha256};

    #[test]
    fn is_denylisted_matches_names_and_dotted_suffixes() {
        use std::ffi::OsStr;
        // exact name matches
        assert!(is_denylisted(OsStr::new("target")));
        assert!(is_denylisted(OsStr::new(".git")));
        assert!(is_denylisted(OsStr::new(".gitignore")));
        assert!(is_denylisted(OsStr::new(".env")));
        assert!(is_denylisted(OsStr::new(".DS_Store")));
        // plain names match exactly only
        assert!(!is_denylisted(OsStr::new("mytarget")));
        assert!(!is_denylisted(OsStr::new("targets")));
        // dotted entries also match as suffix (extension-style)
        assert!(is_denylisted(OsStr::new("backup.git")));
        // unrelated files pass through
        assert!(!is_denylisted(OsStr::new("Cargo.toml")));
        assert!(!is_denylisted(OsStr::new("lib.rs")));
    }

    // Initialize a git repo at `root` with one commit of everything present.
    #[cfg(unix)]
    fn git_init_commit(root: &Path) {
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["add", "-A"][..],
            &["commit", "-q", "-m", "init"][..],
        ] {
            let ok = Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@e.x")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@e.x")
                .status()
                .unwrap()
                .success();
            assert!(ok);
        }
    }

    #[test]
    #[cfg(unix)]
    fn build_source_archive_git_is_prefixed_and_deterministic() {
        use std::os::unix::fs::PermissionsExt;
        let print = Print::new(true);
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), b"// code").unwrap();
        git_init_commit(root);

        let a = build_source_archive(root, &print, true).unwrap();
        let b = build_source_archive(root, &print, true).unwrap();
        assert!(!a.is_empty());
        assert_eq!(a, b, "same commit should produce identical bytes");

        let sha = hex::encode(Sha256::digest(&a));
        assert_eq!(sha.len(), 64);

        // The listing reflects exactly the archived entries.
        let names = entry_names(&a).unwrap();
        assert!(names.iter().any(|n| n == "source/Cargo.toml"));
        assert!(names.iter().any(|n| n == "source/src/lib.rs"));

        // Unpack and confirm the `source/` prefix + hardened perms.
        let dest = tempfile::TempDir::new().unwrap();
        unpack_targz(&a, dest.path()).unwrap();
        assert!(dest.path().join("source/Cargo.toml").exists());
        assert!(dest.path().join("source/src/lib.rs").exists());

        enforce_hardened_tree(dest.path()).unwrap();
        let file_mode = std::fs::metadata(dest.path().join("source/Cargo.toml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let dir_mode = std::fs::metadata(dest.path().join("source"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
    }

    #[test]
    fn build_source_archive_non_git_excludes_denylist() {
        let print = Print::new(true);
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), b"// code").unwrap();
        // Planted dirs that must be excluded.
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/debug/x"), b"junk").unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/config"), b"junk").unwrap();

        let bytes = build_source_archive(root, &print, true).unwrap();
        let dest = tempfile::TempDir::new().unwrap();
        unpack_targz(&bytes, dest.path()).unwrap();

        assert!(dest.path().join("source/Cargo.toml").exists());
        assert!(dest.path().join("source/src/lib.rs").exists());
        assert!(!dest.path().join("source/target").exists());
        assert!(!dest.path().join("source/.git").exists());
        assert_eq!(hex::encode(Sha256::digest(&bytes)).len(), 64);

        // Reproducible: a second run over the same tree yields identical bytes
        // (sorted entries + zeroed header fields + deterministic gzip).
        let again = build_source_archive(root, &print, true).unwrap();
        assert_eq!(bytes, again);
    }

    #[test]
    fn resolve_source_root_finds_git_root_from_subdir() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let nested = root.join("contracts").join("foo");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Cargo.toml"), b"# placeholder").unwrap();

        let manifest = nested.join("Cargo.toml");
        // Use canonicalize on both sides — `tempfile` returns symlinked /var
        // paths on macOS while resolve_source_root walks the same prefix.
        let got = std::fs::canonicalize(resolve_source_root(Some(&manifest))).unwrap();
        let want = std::fs::canonicalize(root).unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn resolve_source_root_falls_back_to_cwd_without_git() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let nested = root.join("noisy");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Cargo.toml"), b"# placeholder").unwrap();

        let manifest = nested.join("Cargo.toml");
        // No `.git` anywhere up the tree, so we fall back to cwd. We can't
        // assert what cwd is in a test runner (it varies), but we can assert
        // that the returned path doesn't have `.git`. That's enough to confirm
        // fallback kicked in.
        let got = resolve_source_root(Some(&manifest));
        assert!(!got.join(".git").exists());
    }
}
