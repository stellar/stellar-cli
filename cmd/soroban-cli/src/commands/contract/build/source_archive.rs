//! Reproducible source-archive generation for verifiable builds.
//!
//! Produces a gzipped tarball of a contract's source tree, rooted under a
//! top-level `source/` prefix (so it extracts to a `source/` dir, mirroring the
//! container's `/source` mount). The working directory is walked and tarred,
//! honoring the project's own `.gitignore`/`.ignore` files (the `.git` directory
//! itself is always skipped). The output is byte-reproducible, so the same tree
//! always hashes to the same `source_sha256`.
//!
//! Shared by `contract build --verifiable` (which builds from the extracted
//! archive) and the standalone `contract archive` command (which generates and
//! inspects it).

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use ignore::WalkBuilder;

use crate::config::{data, locator::enforce_hardened_tree};
use crate::print::Print;

/// Names that usually shouldn't end up in a source archive — VCS metadata of
/// other systems, secrets/local env, build/cache/transient dirs, and editor/OS/
/// AI-assistant junk. These don't *exclude* anything (selection is driven
/// entirely by `.gitignore`/`.ignore`); instead, if any of them slip into the
/// archive because the project didn't ignore them, we warn the user so they can
/// add an ignore rule. Matched against each path component.
pub(crate) const ARCHIVE_WARN_LIST: &[&str] = &[
    // version control (other systems)
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

    #[error(
        "refusing to archive a dirty git working tree at {path}; commit or stash your changes and try again."
    )]
    GitDirty { path: PathBuf },

    #[error("could not write source archive to {path}: {source}")]
    ArchiveWrite {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not extract source archive: {0}")]
    ArchiveExtract(std::io::Error),

    #[error("could not extract source archive: {0}")]
    ZipExtract(zip::result::ZipError),

    #[error(transparent)]
    Data(#[from] data::Error),
}

/// Container formats we can extract a source tree from. This only concerns how
/// the tree is packed for transport; the tree itself is always wrapped in a
/// single top-level directory (SEP-58), which callers check after extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchiveFormat {
    /// Gzipped tarball — what `build --verifiable` produces.
    TarGz,
    /// Zip archive.
    Zip,
}

/// Recognized archive extensions and the format each maps to, matched
/// case-insensitively as a suffix of the archive's filename. Single source of
/// truth for both format detection and the "accepted formats" error text.
const ARCHIVE_EXTENSIONS: &[(&str, ArchiveFormat)] = &[
    (".tar.gz", ArchiveFormat::TarGz),
    (".tgz", ArchiveFormat::TarGz),
    (".zip", ArchiveFormat::Zip),
];

impl ArchiveFormat {
    /// The format named by `filename`'s extension, or `None` if unrecognized.
    pub(crate) fn from_filename(filename: &str) -> Option<Self> {
        let lower = filename.to_ascii_lowercase();
        ARCHIVE_EXTENSIONS
            .iter()
            .find(|(ext, _)| lower.ends_with(ext))
            .map(|(_, format)| *format)
    }

    /// Comma-separated list of accepted extensions, for error messages.
    pub(crate) fn recognized_extensions() -> String {
        ARCHIVE_EXTENSIONS
            .iter()
            .map(|(ext, _)| *ext)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The source tree's root: always the current working directory. The archive is
/// rooted there as-is — we do NOT search upward for a git repository or anchor on
/// `--manifest-path`'s directory, since for a workspace member the build needs
/// the whole workspace (its root `Cargo.toml`/`Cargo.lock`), which lives at the
/// cwd, not the member's directory. So run `contract archive`/`build
/// --verifiable` from the project (or workspace) root you want archived;
/// `--manifest-path`, when given, is interpreted relative to it.
pub(crate) fn resolve_source_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Warn about and reject a dirty git working tree. Both `contract archive` and
/// `build --verifiable` archive the working tree as-is, so uncommitted changes
/// would be baked into the recorded `source_sha256`; refuse them (after
/// explaining why) so an archive always corresponds to a committed state. A
/// no-op when `source_root` isn't a git repo (we can't check, e.g. archive
/// sources) — the user owns the bytes they produce there.
pub(crate) fn ensure_clean_tree(source_root: &Path, print: &Print) -> Result<(), Error> {
    if tree_is_dirty(source_root)? {
        print.warnln(format!(
            "git working tree at {} is dirty; the archive would include uncommitted changes.",
            source_root.display(),
        ));
        return Err(Error::GitDirty {
            path: source_root.to_path_buf(),
        });
    }
    Ok(())
}

/// Whether `source_root` is a git work tree with uncommitted changes. Returns
/// `Ok(false)` when it isn't a git repo (git ran but refused) — callers can't
/// verify cleanliness there, so they proceed. Errors only when git can't be
/// invoked at all.
fn tree_is_dirty(source_root: &Path) -> Result<bool, Error> {
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

/// Produce the gzipped source tarball bytes. The working directory under
/// `source_root` is walked and tarred, honoring the project's `.gitignore`/
/// `.ignore` files; entries are rooted under a top-level `source/` prefix.
///
/// `warn` controls whether to warn about archived paths that usually shouldn't
/// be shipped (see `ARCHIVE_WARN_LIST`). Callers that only inspect the result
/// (e.g. `contract archive --dry-run`) pass `false`, since the listing itself
/// reveals the contents.
pub(crate) fn build_source_archive(
    source_root: &Path,
    print: &Print,
    warn: bool,
) -> Result<Vec<u8>, Error> {
    let tar = walk_tar(source_root, print, warn)?;
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

/// Tar the working tree under `source_root`, honoring the project's `.gitignore`/
/// `.ignore` files and always skipping the `.git` directory. Each entry is
/// prefixed with `source/`. When `warn` is set, archived paths matching
/// `ARCHIVE_WARN_LIST` (e.g. `.env`, `target/`) trigger a warning so the user can
/// add an ignore rule.
///
/// Selection depends only on the in-tree files plus the `.gitignore`/`.ignore`
/// files inside the archived tree — never on machine-specific state (the global
/// gitignore, `.git/info/exclude`, or ignore files in parent directories are not
/// consulted) — so the archive stays byte-reproducible across machines.
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
fn walk_tar(source_root: &Path, print: &Print, warn: bool) -> Result<Vec<u8>, Error> {
    let walk = WalkBuilder::new(source_root)
        .hidden(false) // include dotfiles; let .gitignore decide
        .git_ignore(true) // honor in-tree .gitignore
        .ignore(true) // honor .ignore
        .git_global(false) // not the machine's global gitignore (not reproducible)
        .git_exclude(false) // not .git/info/exclude (not in the archive)
        .require_git(false) // apply .gitignore/.ignore even without a .git dir
        .parents(false) // only ignore files inside the archived tree
        .filter_entry(|e| e.file_name() != ".git") // never archive VCS internals
        .build();

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in walk {
        let entry = entry.map_err(|source| Error::ArchiveWrite {
            path: source_root.to_path_buf(),
            source: std::io::Error::other(source),
        })?;
        if entry.file_type().is_some_and(|t| t.is_file()) {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();

    if warn {
        warn_unexpected_paths(&files, source_root, print);
    }

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

/// Whether a path component matches the warn list: it equals an entry, or — for
/// dotted entries, which double as extension filters (e.g. `.swp`, `.log`) — it
/// ends with that entry. Plain names (`target`, `node_modules`) match exactly
/// only, so `mytarget` is not flagged.
fn is_warned(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    ARCHIVE_WARN_LIST
        .iter()
        .any(|d| name == *d || (d.starts_with('.') && name.ends_with(d)))
}

/// Warn about archived paths that usually shouldn't be shipped (secrets, build
/// output, editor/OS junk; see `ARCHIVE_WARN_LIST`). Selection is driven by
/// `.gitignore`/`.ignore`, so these slipped in only because the project didn't
/// ignore them — point that out so the user can add a rule. Reports the path up
/// to each matched component once (so a flagged directory is named once, not per
/// file under it), each on its own line since paths can be long.
fn warn_unexpected_paths(files: &[PathBuf], source_root: &Path, print: &Print) {
    let mut hits: Vec<String> = Vec::new();
    for path in files {
        let rel = path.strip_prefix(source_root).unwrap_or(path);
        let mut prefix = PathBuf::new();
        for comp in rel.components() {
            prefix.push(comp);
            if is_warned(comp.as_os_str()) {
                let hit = prefix.to_string_lossy().into_owned();
                if !hits.contains(&hit) {
                    hits.push(hit);
                }
                break;
            }
        }
    }
    if hits.is_empty() {
        return;
    }
    hits.sort();
    print.warnln(
        "archive includes paths usually excluded; add them to .gitignore or .ignore if unintended:",
    );
    for hit in &hits {
        print.blankln(hit);
    }
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

/// Unpack a zip archive into `dest`. `ZipArchive::extract` sanitizes each
/// entry's path (dropping anything that would escape `dest`), so a hostile
/// archive can't write outside the tempdir.
pub(crate) fn unpack_zip(bytes: &[u8], dest: &Path) -> Result<(), Error> {
    zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .and_then(|mut archive| archive.extract(dest))
        .map_err(Error::ZipExtract)
}

/// Create a fresh temp directory, unpack the source archive `bytes` (in the
/// given `format`) into it, harden its permissions, and return the guard (the
/// tree lives at its `path()`). Shared by `build --verifiable` (builds from the
/// extracted copy) and `verify` (rebuilds from it); `prefix` names the dir so
/// the two are distinguishable on disk.
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
    format: ArchiveFormat,
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
    match format {
        ArchiveFormat::TarGz => unpack_targz(bytes, tmp.path())?,
        ArchiveFormat::Zip => unpack_zip(bytes, tmp.path())?,
    }
    enforce_hardened_tree(tmp.path()).map_err(Error::ArchiveExtract)?;
    Ok(tmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::locator::enforce_hardened_tree;
    use sha2::{Digest, Sha256};

    #[test]
    fn archive_format_from_filename() {
        assert_eq!(
            ArchiveFormat::from_filename("src.tar.gz"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            ArchiveFormat::from_filename("SRC.TGZ"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            ArchiveFormat::from_filename("src.zip"),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(ArchiveFormat::from_filename("src.rar"), None);
        assert_eq!(ArchiveFormat::from_filename("src"), None);
        // The listed extensions are exactly what the error surfaces.
        assert_eq!(
            ArchiveFormat::recognized_extensions(),
            ".tar.gz, .tgz, .zip"
        );
    }

    #[test]
    fn unpack_zip_round_trips() {
        use std::io::Write;
        // Build a small zip with a single top-level `source/` dir.
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            zip.start_file("source/Cargo.toml", opts).unwrap();
            zip.write_all(b"# crate").unwrap();
            zip.start_file("source/src/lib.rs", opts).unwrap();
            zip.write_all(b"// code").unwrap();
            zip.finish().unwrap();
        }

        let dest = tempfile::TempDir::new().unwrap();
        unpack_zip(&buf, dest.path()).unwrap();
        assert_eq!(
            std::fs::read(dest.path().join("source/Cargo.toml")).unwrap(),
            b"# crate"
        );
        assert_eq!(
            std::fs::read(dest.path().join("source/src/lib.rs")).unwrap(),
            b"// code"
        );
    }

    #[test]
    fn is_warned_matches_names_and_dotted_suffixes() {
        use std::ffi::OsStr;
        // exact name matches
        assert!(is_warned(OsStr::new("target")));
        assert!(is_warned(OsStr::new(".env")));
        assert!(is_warned(OsStr::new(".DS_Store")));
        // plain names match exactly only
        assert!(!is_warned(OsStr::new("mytarget")));
        assert!(!is_warned(OsStr::new("targets")));
        // dotted entries also match as suffix (extension-style)
        assert!(is_warned(OsStr::new("backup.svn")));
        // `.git`/`.gitignore` are not warned: `.git` is skipped structurally and
        // `.gitignore` is legitimately archived like any other tracked file.
        assert!(!is_warned(OsStr::new(".git")));
        assert!(!is_warned(OsStr::new(".gitignore")));
        // unrelated files pass through
        assert!(!is_warned(OsStr::new("Cargo.toml")));
        assert!(!is_warned(OsStr::new("lib.rs")));
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
        assert_eq!(a, b, "same tree should produce identical bytes");

        // The `.git` dir git_init_commit created is never archived.
        assert!(entry_names(&a)
            .unwrap()
            .iter()
            .all(|n| !n.starts_with("source/.git/")));

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
    fn build_source_archive_skips_git_dir_and_is_reproducible() {
        let print = Print::new(true);
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), b"// code").unwrap();
        // A `.git` dir is always skipped, even without a real repo.
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/config"), b"junk").unwrap();
        // No `.gitignore`, so `target/` is NOT excluded — selection is driven by
        // ignore files only.
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/debug/x"), b"junk").unwrap();

        let bytes = build_source_archive(root, &print, true).unwrap();
        let dest = tempfile::TempDir::new().unwrap();
        unpack_targz(&bytes, dest.path()).unwrap();

        assert!(dest.path().join("source/Cargo.toml").exists());
        assert!(dest.path().join("source/src/lib.rs").exists());
        assert!(!dest.path().join("source/.git").exists());
        // Un-ignored `target/` is included (and would have triggered a warning).
        assert!(dest.path().join("source/target/debug/x").exists());
        assert_eq!(hex::encode(Sha256::digest(&bytes)).len(), 64);

        // Reproducible: a second run over the same tree yields identical bytes
        // (sorted entries + zeroed header fields + deterministic gzip).
        let again = build_source_archive(root, &print, true).unwrap();
        assert_eq!(bytes, again);
    }

    #[test]
    fn build_source_archive_respects_gitignore_and_dot_ignore() {
        let print = Print::new(true);
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), b"// code").unwrap();
        // `.gitignore` and `.ignore` are honored even without a git repo.
        std::fs::write(root.join(".gitignore"), b"target/\n").unwrap();
        std::fs::write(root.join(".ignore"), b"secret.txt\n").unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::write(root.join("target/debug/x"), b"junk").unwrap();
        std::fs::write(root.join("secret.txt"), b"shh").unwrap();

        let bytes = build_source_archive(root, &print, true).unwrap();
        let dest = tempfile::TempDir::new().unwrap();
        unpack_targz(&bytes, dest.path()).unwrap();

        assert!(dest.path().join("source/Cargo.toml").exists());
        assert!(dest.path().join("source/src/lib.rs").exists());
        // Excluded by the in-tree ignore files.
        assert!(!dest.path().join("source/target").exists());
        assert!(!dest.path().join("source/secret.txt").exists());
        // The ignore files themselves are archived like any other tracked file.
        assert!(dest.path().join("source/.gitignore").exists());
    }

    #[test]
    fn resolve_source_root_is_cwd() {
        // The root is always the current working directory — no upward search,
        // no manifest anchoring.
        assert_eq!(resolve_source_root(), std::env::current_dir().unwrap());
    }
}
