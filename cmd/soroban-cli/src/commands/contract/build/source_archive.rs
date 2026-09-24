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
//! archive) and the `contract build archive` command (which generates and
//! inspects it).

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use ignore::WalkBuilder;
use soroban_spec_tools::sanitize;

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

    #[error("could not check the git working tree at {path}: {stderr}")]
    GitStatus { path: PathBuf, stderr: String },

    #[error(
        "refusing to archive a dirty git working tree at {path}; commit or stash your changes and try again."
    )]
    GitDirty { path: PathBuf },

    #[error(
        "refusing to archive: {paths:?} marked assume-unchanged or skip-worktree, so git can't confirm they match the committed source; clear the flag (git update-index --no-assume-unchanged / --no-skip-worktree <file>) and try again."
    )]
    GitUnverifiable { paths: Vec<PathBuf> },

    #[error(
        "refusing to archive: submodule(s) {paths:?} are not initialized, so their committed source would be missing from the archive; run `git submodule update --init --recursive` and try again."
    )]
    SubmoduleUninitialized { paths: Vec<PathBuf> },

    #[error("could not write source archive to {path:?}: {source}")]
    ArchiveWrite {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not extract source archive: {0}")]
    ArchiveExtract(std::io::Error),

    #[error(
        "refusing to archive symlink {link:?}: symlinks are not supported in a reproducible source archive; replace it with the real file (or ignore it via .gitignore/.ignore) and try again."
    )]
    Symlink { link: PathBuf },
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

/// Reject a dirty git working tree. Both `contract archive` and `build
/// --verifiable` archive the working tree as-is, so uncommitted changes would be
/// baked into the recorded `source_sha256`; refuse them so an archive always
/// corresponds to a committed state. A no-op when `source_root` isn't a git repo
/// (we can't check, e.g. archive sources) — the user owns the bytes they produce
/// there.
///
/// `exclude` is the caller's own output file, kept out of the check exactly as
/// it's kept out of the archive, so re-running over an unchanged tree that
/// already holds a previous tarball isn't seen as dirty.
pub(crate) fn ensure_clean_tree(source_root: &Path, exclude: Option<&Path>) -> Result<(), Error> {
    let selected = collect_files(source_root, exclude)?;

    // An uninitialized submodule is an empty dir the walker archives nothing for,
    // yet its committed source belongs in the archive — reject rather than hash an
    // incomplete tree.
    let uninitialized = uninitialized_submodules(source_root)?;
    if !uninitialized.is_empty() {
        return Err(Error::SubmoduleUninitialized {
            paths: uninitialized,
        });
    }

    // Files git has been told to ignore working-tree changes for can't be
    // verified by the dirty check below, so reject them first (more specific).
    let unverifiable = unverifiable_files(source_root, &selected)?;
    if !unverifiable.is_empty() {
        return Err(Error::GitUnverifiable {
            paths: unverifiable,
        });
    }

    if tree_is_dirty(source_root, &selected)? {
        return Err(Error::GitDirty {
            path: source_root.to_path_buf(),
        });
    }
    Ok(())
}

/// Whether `source_root` is a git work tree that isn't safe to archive. Checked
/// against the *archived* file set (`selected`), not git's default status
/// filtering, because the walker and `git status` apply different ignore rules —
/// the walker skips the global gitignore, `.git/info/exclude`, and parent-dir
/// ignores, and additionally honors `.ignore` — so a file could be archived
/// while status still called the tree clean (or the reverse). A tree is dirty
/// when either a tracked file is modified/staged/deleted, or a file the archive
/// would include isn't committed. Returns `Ok(false)` when it isn't a git repo
/// (nothing to verify). Errors only when git can't be invoked or fails
/// otherwise.
fn tree_is_dirty(source_root: &Path, selected: &[PathBuf]) -> Result<bool, Error> {
    // Modified/staged/deleted tracked files. `--untracked-files=no` keeps this
    // independent of ignore rules; untracked files are covered by the
    // committed-membership check below instead.
    // `--ignore-submodules=none` overrides any `submodule.<name>.ignore` /
    // `diff.ignoreSubmodules` config that would otherwise hide a submodule's
    // modified tracked files, whose changed bytes the walker still archives.
    let Some(status) = run_git(
        source_root,
        &[
            "status",
            "--porcelain",
            "--untracked-files=no",
            "--ignore-submodules=none",
        ],
    )?
    else {
        return Ok(false); // not a git repo — nothing to verify
    };
    if !status.is_empty() {
        return Ok(true);
    }

    // Every file the archive would include must be committed; otherwise the
    // archive bakes in uncommitted content while the status check above still
    // saw a clean tree (e.g. a file hidden from status by a global/`info/exclude`
    // ignore that the walker doesn't consult).
    let tracked = tracked_files(source_root)?;
    Ok(selected
        .iter()
        .any(|path| !tracked.contains(path.strip_prefix(source_root).unwrap_or(path))))
}

/// Run `git -C source_root <args>` under the C locale. Returns the captured
/// stdout on success, `None` when `source_root` isn't a git repository (nothing
/// to verify there), or an error for any other failure. git exits non-zero
/// (typically 128) for both "not a git repository" and genuine failures —
/// dubious ownership, permission errors, a corrupt repo — so the first is
/// distinguished by its (C-locale, hence stable English) message; the rest are
/// surfaced rather than silently treated as "not a repo".
fn run_git(source_root: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, Error> {
    let output = Command::new("git")
        .env("LC_ALL", "C")
        .arg("-C")
        .arg(source_root)
        .args(args)
        .output()
        .map_err(|source| Error::GitInvoke {
            path: source_root.to_path_buf(),
            source,
        })?;

    if output.status.success() {
        return Ok(Some(output.stdout));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("not a git repository") {
        return Ok(None);
    }
    Err(Error::GitStatus {
        path: source_root.to_path_buf(),
        stderr: stderr.trim().to_string(),
    })
}

/// The set of tracked files under `source_root`, as paths relative to it.
/// `--recurse-submodules` descends into initialized submodules (whose working
/// files the walker also archives, but which `ls-files` would otherwise report
/// only as a single gitlink path), so a clean project using a submodule isn't
/// mistaken for dirty.
fn tracked_files(source_root: &Path) -> Result<std::collections::HashSet<PathBuf>, Error> {
    let out =
        run_git(source_root, &["ls-files", "-z", "--recurse-submodules"])?.unwrap_or_default();
    // `-z` gives NUL-separated, unquoted paths — so a name with spaces or other
    // special bytes still matches the walker's real path.
    Ok(out
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(bytes_to_path)
        .collect())
}

/// Files whose index flags tell git to ignore their working-tree state, so we
/// can't confirm they match committed source: `assume-unchanged` (a lowercased
/// `git ls-files -v` tag) and `skip-worktree` (tag `S`/`s`).
///
/// `assume-unchanged` files are on disk, so they only matter when archived —
/// gated on `selected`. `skip-worktree` files may be absent from disk (sparse
/// checkout), so they never reach `selected`; reject every one regardless, since
/// their committed source can't be archived either way. Empty when `source_root`
/// isn't a git repo.
fn unverifiable_files(source_root: &Path, selected: &[PathBuf]) -> Result<Vec<PathBuf>, Error> {
    // `--recurse-submodules` so a flagged file inside an initialized submodule
    // (which the walker archives) is caught too, matching `tracked_files`.
    let Some(out) = run_git(
        source_root,
        &["ls-files", "-v", "-z", "--recurse-submodules"],
    )?
    else {
        return Ok(Vec::new());
    };
    let selected: std::collections::HashSet<&Path> = selected
        .iter()
        .map(|p| p.strip_prefix(source_root).unwrap_or(p))
        .collect();

    // Each record is `<tag><space><path>` (see `git ls-files -v`); the path
    // starts after the tag and its separating space.
    let mut unverifiable = Vec::new();
    for record in out.split(|b| *b == 0).filter(|r| r.len() > 2) {
        let tag = record[0];
        let path = bytes_to_path(&record[2..]);
        let is_skip_worktree = tag == b'S' || tag == b's';
        let is_assume_unchanged = tag.is_ascii_lowercase();
        if is_skip_worktree || (is_assume_unchanged && selected.contains(path.as_path())) {
            unverifiable.push(path);
        }
    }
    Ok(unverifiable)
}

/// Submodule paths that are present as gitlinks but not checked out. `git
/// submodule status --recursive` prefixes such entries with `-`; their working
/// dirs are empty, so the walker archives none of their (committed) source.
/// Empty when `source_root` isn't a git repo or has no uninitialized submodules.
fn uninitialized_submodules(source_root: &Path) -> Result<Vec<PathBuf>, Error> {
    let Some(out) = run_git(source_root, &["submodule", "status", "--recursive"])? else {
        return Ok(Vec::new());
    };
    // Each line is `<flag><sha> <path> (<describe>)`; `-` flags an uninitialized
    // submodule, and the path is the second whitespace-separated token.
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|l| {
            l.strip_prefix('-')
                .and_then(|rest| rest.split_whitespace().nth(1))
        })
        .map(PathBuf::from)
        .collect())
}

fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Produce the gzipped source tarball bytes. The working directory under
/// `source_root` is walked and tarred, honoring the project's `.gitignore`/
/// `.ignore` files; entries are rooted under a top-level `source/` prefix.
///
/// `warn` controls whether to warn about archived paths that usually shouldn't
/// be shipped (see `ARCHIVE_WARN_LIST`). Callers that only inspect the result
/// (e.g. `contract archive --dry-run`) pass `false`, since the listing itself
/// reveals the contents.
///
/// `exclude` is a single path to skip during the walk — the caller's own output
/// file (`contract archive --out-file`), so re-running over an unchanged tree
/// that already contains a previous tarball doesn't archive it into the new one.
pub(crate) fn build_source_archive(
    source_root: &Path,
    print: &Print,
    warn: bool,
    exclude: Option<&Path>,
) -> Result<Vec<u8>, Error> {
    let tar = walk_tar(source_root, print, warn, exclude)?;
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
fn walk_tar(
    source_root: &Path,
    print: &Print,
    warn: bool,
    exclude: Option<&Path>,
) -> Result<Vec<u8>, Error> {
    let files = collect_files(source_root, exclude)?;

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

/// The sorted set of files the archive would contain: the working tree under
/// `source_root`, honoring the project's in-tree `.gitignore`/`.ignore` (and
/// only those — see `walk_tar`), with the `.git` directory and the caller's own
/// `exclude` output file skipped. Rejects symlinks. This is the single source of
/// truth for "what goes in the archive", shared by `walk_tar` (to build it) and
/// `ensure_clean_tree` (to check the same files are committed).
fn collect_files(source_root: &Path, exclude: Option<&Path>) -> Result<Vec<PathBuf>, Error> {
    // Resolve the excluded output file to its real path (only when it already
    // exists — a not-yet-written file can't be in the tree to skip).
    let exclude = exclude.and_then(|p| p.canonicalize().ok());

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
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        // A symlink is neither followed (its target could sit outside the tree,
        // pulling in machine-specific content and breaking source_sha256) nor
        // stored as a link entry; reject it so the archive is always a faithful,
        // reproducible snapshot of real files.
        if file_type.is_symlink() {
            return Err(Error::Symlink {
                link: entry.path().to_path_buf(),
            });
        }
        if file_type.is_file() {
            let path = entry.path();
            // Skip our own output file (a prior run's tarball); pre-filter on the
            // file name so we only canonicalize the rare same-named candidate.
            if let Some(ex) = &exclude {
                if path.file_name() == ex.file_name()
                    && path.canonicalize().ok().as_deref() == Some(ex.as_path())
                {
                    continue;
                }
            }
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    Ok(files)
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
    // Hits are built from scanned filename components, so sanitize control/escape
    // bytes before printing to keep a hostile filename from injecting into the
    // terminal.
    for hit in &hits {
        print.blankln(sanitize(hit));
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::config::locator::{enforce_hardened_tree, FileMode};
    use sha2::{Digest, Sha256};

    /// Decompress gzip and unpack the tar into `dest`. Entries are `source/…`,
    /// so they land at `<dest>/source/…`.
    fn unpack_targz(bytes: &[u8], dest: &Path) -> Result<(), Error> {
        let dec = flate2::read::GzDecoder::new(bytes);
        tar::Archive::new(dec)
            .unpack(dest)
            .map_err(Error::ArchiveExtract)
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

    // Run a single git command in `root`, asserting it succeeds.
    #[cfg(unix)]
    fn git_run(root: &Path, args: &[&str]) {
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
        assert!(ok, "git {args:?} failed");
    }

    // Initialize a git repo at `root` with one commit of everything present.
    #[cfg(unix)]
    fn git_init_commit(root: &Path) {
        git_run(root, &["init", "-q", "-b", "main"]);
        git_run(root, &["add", "-A"]);
        git_run(root, &["commit", "-q", "-m", "init"]);
    }

    // A committed superproject with one committed submodule at `sub/`. Returns the
    // superproject's tempdir, the submodule's tempdir (kept alive so its origin
    // path stays valid), and the superproject root.
    #[cfg(unix)]
    fn superproject_with_submodule() -> (tempfile::TempDir, tempfile::TempDir, PathBuf) {
        let sub = tempfile::TempDir::new().unwrap();
        std::fs::write(sub.path().join("f.txt"), b"// sub").unwrap();
        git_init_commit(sub.path());

        // `protocol.file.allow` is required for a local-path submodule on modern git.
        let super_dir = tempfile::TempDir::new().unwrap();
        let root = super_dir.path().to_path_buf();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        git_run(&root, &["init", "-q", "-b", "main"]);
        git_run(
            &root,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                &sub.path().to_string_lossy(),
                "sub",
            ],
        );
        git_run(&root, &["add", "-A"]);
        git_run(&root, &["commit", "-q", "-m", "init"]);
        (super_dir, sub, root)
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

        let a = build_source_archive(root, &print, true, None).unwrap();
        let b = build_source_archive(root, &print, true, None).unwrap();
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

        enforce_hardened_tree(dest.path(), FileMode::PreserveOwner).unwrap();
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

        let bytes = build_source_archive(root, &print, true, None).unwrap();
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
        let again = build_source_archive(root, &print, true, None).unwrap();
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

        let bytes = build_source_archive(root, &print, true, None).unwrap();
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

    // A previous run's tarball sitting inside the tree must be excluded, so
    // re-archiving an otherwise-unchanged tree doesn't nest the old archive.
    #[test]
    fn build_source_archive_excludes_the_output_file() {
        let print = Print::new(true);
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        let out = root.join("snapshot.tar.gz");
        std::fs::write(&out, b"a previous run's archive").unwrap();

        // With the output excluded, it isn't archived; real source still is.
        let names =
            entry_names(&build_source_archive(root, &print, false, Some(&out)).unwrap()).unwrap();
        assert!(names.iter().any(|n| n == "source/Cargo.toml"));
        assert!(
            !names.iter().any(|n| n.ends_with("snapshot.tar.gz")),
            "the output file must not be archived into itself: {names:?}"
        );

        // Control: without excluding it, the stray tarball would be included.
        let included =
            entry_names(&build_source_archive(root, &print, false, None).unwrap()).unwrap();
        assert!(included.iter().any(|n| n.ends_with("snapshot.tar.gz")));
    }

    #[test]
    fn resolve_source_root_is_cwd() {
        // The root is always the current working directory — no upward search,
        // no manifest anchoring.
        assert_eq!(resolve_source_root(), std::env::current_dir().unwrap());
    }

    // A file the archive would include but git doesn't track must fail the
    // clean-tree check, so uncommitted content never lands in a "clean" archive.
    // Here `secret.rs` is hidden from `git status` via `.git/info/exclude` — which
    // the walker deliberately ignores — so the old status-only check called the
    // tree clean while the walker still archived it.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_rejects_archived_but_uncommitted_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        git_init_commit(root);

        std::fs::write(root.join(".git/info/exclude"), b"secret.rs\n").unwrap();
        std::fs::write(root.join("secret.rs"), b"// uncommitted").unwrap();

        let err = ensure_clean_tree(root, None).unwrap_err();
        assert!(matches!(err, Error::GitDirty { .. }), "got {err:?}");
    }

    // The caller's own output file, sitting untracked inside the repo, must not
    // trip the clean-tree check when it's the excluded output — otherwise a second
    // `archive -o inside.tar.gz` run would wrongly fail as dirty. Not excluding it
    // proves the check does otherwise catch an untracked file.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_ignores_the_excluded_output_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        git_init_commit(root);

        let out = root.join("src.tar.gz");
        std::fs::write(&out, b"a prior run's archive").unwrap();

        ensure_clean_tree(root, Some(&out))
            .expect("the excluded output file must not count as dirty");
        let err = ensure_clean_tree(root, None).unwrap_err();
        assert!(matches!(err, Error::GitDirty { .. }), "got {err:?}");
    }

    // A modified *tracked* file is dirty even though the committed-membership
    // check alone would pass it (it's tracked) — the status probe catches it.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_rejects_modified_tracked_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        git_init_commit(root);

        std::fs::write(root.join("Cargo.toml"), b"# modified").unwrap();

        let err = ensure_clean_tree(root, None).unwrap_err();
        assert!(matches!(err, Error::GitDirty { .. }), "got {err:?}");
    }

    // A file marked `assume-unchanged` is skipped by `git status`/`git diff`, so a
    // modification to it would be archived while looking clean. We can't vouch it
    // matches committed source, so it must be refused.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_rejects_assume_unchanged_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        git_init_commit(root);

        git_run(root, &["update-index", "--assume-unchanged", "Cargo.toml"]);
        std::fs::write(root.join("Cargo.toml"), b"# modified out of view").unwrap();

        let err = ensure_clean_tree(root, None).unwrap_err();
        assert!(matches!(err, Error::GitUnverifiable { .. }), "got {err:?}");
    }

    // Same guarantee for `skip-worktree`, the other index flag that hides
    // working-tree changes from git.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_rejects_skip_worktree_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        git_init_commit(root);

        git_run(root, &["update-index", "--skip-worktree", "Cargo.toml"]);
        std::fs::write(root.join("Cargo.toml"), b"# modified out of view").unwrap();

        let err = ensure_clean_tree(root, None).unwrap_err();
        assert!(matches!(err, Error::GitUnverifiable { .. }), "got {err:?}");
    }

    // A `skip-worktree` file absent from disk (e.g. a sparse checkout) never
    // reaches the walker's selected set, but its committed source still belongs in
    // the archive — so it must be rejected, not silently dropped.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_rejects_absent_skip_worktree_file() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        std::fs::write(root.join("extra.rs"), b"// committed").unwrap();
        git_init_commit(root);

        git_run(root, &["update-index", "--skip-worktree", "extra.rs"]);
        std::fs::remove_file(root.join("extra.rs")).unwrap();

        let err = ensure_clean_tree(root, None).unwrap_err();
        assert!(matches!(err, Error::GitUnverifiable { .. }), "got {err:?}");
    }

    // A committed, unmodified tree is clean.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_accepts_committed_tree() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), b"// code").unwrap();
        git_init_commit(root);

        ensure_clean_tree(root, None).expect("a committed tree is clean");
    }

    // A clean project that embeds an initialized git submodule must pass: the
    // walker archives the submodule's files, so the tracked set has to include
    // them too (via `--recurse-submodules`) — otherwise they look untracked and
    // the tree is wrongly rejected as dirty.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_accepts_committed_submodule() {
        let (_super, _sub, root) = superproject_with_submodule();
        ensure_clean_tree(&root, None).expect("a committed submodule must be clean");
    }

    // An uninitialized submodule is an empty dir: the walker archives nothing for
    // it, so the archive would silently omit its committed source. Reject it.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_rejects_uninitialized_submodule() {
        let (_super, _sub, root) = superproject_with_submodule();
        git_run(&root, &["submodule", "deinit", "-f", "sub"]);

        let err = ensure_clean_tree(&root, None).unwrap_err();
        assert!(
            matches!(err, Error::SubmoduleUninitialized { .. }),
            "got {err:?}"
        );
    }

    // A submodule configured `ignore = all` hides its modified tracked files from
    // `git status`, but the walker still archives the changed bytes. The check must
    // override that config (`--ignore-submodules=none`) and catch it.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_rejects_modified_ignored_submodule() {
        let (_super, _sub, root) = superproject_with_submodule();
        git_run(&root, &["config", "submodule.sub.ignore", "all"]);
        std::fs::write(root.join("sub/f.txt"), b"// modified out of view").unwrap();

        let err = ensure_clean_tree(&root, None).unwrap_err();
        assert!(matches!(err, Error::GitDirty { .. }), "got {err:?}");
    }

    // A submodule file marked `assume-unchanged` is hidden from status; the flag
    // query must recurse into submodules to catch it, else its modified bytes get
    // archived while the tree looks clean.
    #[test]
    #[cfg(unix)]
    fn ensure_clean_tree_rejects_assume_unchanged_submodule_file() {
        let (_super, _sub, root) = superproject_with_submodule();
        git_run(
            &root.join("sub"),
            &["update-index", "--assume-unchanged", "f.txt"],
        );
        std::fs::write(root.join("sub/f.txt"), b"// modified out of view").unwrap();

        let err = ensure_clean_tree(&root, None).unwrap_err();
        assert!(matches!(err, Error::GitUnverifiable { .. }), "got {err:?}");
    }

    // A symlink in the tree is rejected rather than followed (its target could be
    // outside the tree, breaking reproducibility) or stored as a link entry.
    #[test]
    #[cfg(unix)]
    fn build_source_archive_rejects_symlinks() {
        let print = Print::new(true);
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        std::os::unix::fs::symlink("Cargo.toml", root.join("link.toml")).unwrap();

        let err = build_source_archive(root, &print, false, None).unwrap_err();
        assert!(matches!(err, Error::Symlink { .. }), "got {err:?}");
    }

    // A symlink filename is working-tree-controlled, so a hostile repo could put
    // terminal escape bytes in it. The rejection error must escape them, or
    // `archive --dry-run` would emit raw control sequences before the sanitized
    // listing is ever reached.
    #[test]
    #[cfg(unix)]
    fn symlink_error_escapes_control_bytes_in_name() {
        use std::os::unix::ffi::OsStrExt;
        let print = Print::new(true);
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        std::fs::write(root.join("Cargo.toml"), b"# crate").unwrap();
        // `e` + raw ESC + ANSI color sequence + `vil`.
        let evil = std::ffi::OsStr::from_bytes(b"e\x1b[31mvil");
        std::os::unix::fs::symlink("Cargo.toml", root.join(evil)).unwrap();

        let err = build_source_archive(root, &print, false, None).unwrap_err();
        assert!(
            !err.to_string().contains('\u{1b}'),
            "raw ESC leaked into the symlink error: {:?}",
            err.to_string()
        );
    }

    // Hardening the extracted tree strips group/other access but must keep the
    // owner execute bit, so a checked-in script a build invokes stays runnable.
    #[test]
    #[cfg(unix)]
    fn hardening_preserves_owner_execute_bit() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();

        let script = root.join("build.sh");
        std::fs::write(&script, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let data = root.join("data.txt");
        std::fs::write(&data, b"x").unwrap();
        std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o644)).unwrap();

        enforce_hardened_tree(root, FileMode::PreserveOwner).unwrap();

        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        // Executable file keeps owner-exec (0700); non-exec file hardened to 0600;
        // group/other stripped in both.
        assert_eq!(mode(&script), 0o700, "exec bit must survive hardening");
        assert_eq!(mode(&data), 0o600);
    }
}
