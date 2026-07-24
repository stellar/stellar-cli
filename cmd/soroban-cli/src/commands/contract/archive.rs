use std::path::PathBuf;

use clap::Parser;
use sha2::{Digest, Sha256};

use crate::{commands::global, print::Print};

use super::build::source_archive;

/// Accepted `--out-file` suffixes (lower-case). The archive is always a gzipped
/// tarball, so the filename must say so.
const ARCHIVE_EXTENSIONS: &[&str] = &[".tar.gz", ".tgz"];

/// Generate (or inspect) the reproducible source archive for a contract.
///
/// Produces the same gzipped tarball that `stellar contract build --verifiable`
/// builds from, and prints its SHA-256 (the SEP-58 `source_sha256`). Use
/// `--dry-run` to list exactly what would be archived without writing anything —
/// handy for confirming the contents before a verifiable build, or for
/// producing the archive to host at a `--source-uri`.
///
/// The archive is the current working directory, honoring the project's
/// `.gitignore` and `.ignore` files (the `.git` directory itself is always
/// skipped). Run this from the project (or workspace) root you want archived.
#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Where to write the gzipped tarball. Required unless `--dry-run` is used.
    #[arg(long, short = 'o', required_unless_present = "dry_run")]
    pub out_file: Option<PathBuf>,

    /// List the entries that would be archived and the computed source_sha256,
    /// without writing any file.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    SourceArchive(#[from] source_archive::Error),

    #[error(
        "--out-file {0} must end in .tar.gz or .tgz (the archive is always a gzipped tarball)"
    )]
    OutFileExtension(String),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        let source_root = source_archive::resolve_source_root();

        // The archive is the working tree, so a dirty repo would bake uncommitted
        // changes into the bytes and the printed source_sha256 — refuse it, so the
        // hash always corresponds to a committed state (matching --verifiable).
        source_archive::ensure_clean_tree(&source_root, &print)?;

        // The dry-run listing itself reveals the contents, so skip the
        // "not a git repository" warning there.
        let bytes = source_archive::build_source_archive(&source_root, &print, !self.dry_run)?;
        let sha = hex::encode(Sha256::digest(&bytes));

        if self.dry_run {
            let names = source_archive::entry_names(&bytes)?;
            let prefix = print.compute_emoji("📄");

            for name in &names {
                println!("{prefix} {name}");
            }
            print.infoln(format!("{} files", names.len()));
            print.infoln(format!("source_sha256 {sha}"));
            return Ok(());
        }

        // `--out-file` is required when not `--dry-run`, so this is always set here.
        let out = self
            .out_file
            .as_ref()
            .expect("--out-file is required without --dry-run");

        // The output is always a gzipped tarball, so require a matching
        // extension to keep the filename honest.
        let name = out
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        if !ARCHIVE_EXTENSIONS.iter().any(|ext| name.ends_with(ext)) {
            return Err(Error::OutFileExtension(out.display().to_string()));
        }

        if let Some(parent) = out.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|source| {
                    source_archive::Error::ArchiveWrite {
                        path: out.clone(),
                        source,
                    }
                })?;
            }
        }
        std::fs::write(out, &bytes).map_err(|source| source_archive::Error::ArchiveWrite {
            path: out.clone(),
            source,
        })?;
        print.checkln(format!(
            "Wrote source archive {} (source_sha256 {sha})",
            out.display()
        ));

        Ok(())
    }
}
