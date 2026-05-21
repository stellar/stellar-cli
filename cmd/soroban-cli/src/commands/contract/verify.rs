use std::path::PathBuf;

use clap::Parser;
use soroban_spec_tools::contract::Spec;
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

use crate::{
    commands::{
        contract::build::verifiable::{
            bldimg_regex, source_repo_regex, source_rev_regex, tarball_sha256_regex,
            tarball_url_regex,
        },
        global,
    },
    config::{self, locator, network},
    print::Print,
    wasm,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Contract id or alias to fetch the WASM from the network.
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID", conflicts_with = "wasm")]
    pub contract_id: Option<config::UnresolvedContract>,

    /// Local WASM file to verify, instead of fetching from the network.
    #[arg(long)]
    pub wasm: Option<PathBuf>,

    /// Local tarball file or http(s) URL to use as the source when the WASM's
    /// recorded SEP-58 metadata has only `tarball_sha256` (no `tarball_url`).
    /// Accepts http(s) URLs or local file paths.
    #[arg(long)]
    pub tarball_url: Option<String>,

    /// Bypass interactive confirmation when the WASM's bldimg is not in the
    /// default trust list, or when the source is a tarball (tarballs are
    /// never default-trusted).
    #[arg(long)]
    pub trust: bool,

    #[command(flatten)]
    pub locator: locator::Args,

    #[command(flatten)]
    pub network: network::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("must pass exactly one of --id or --wasm")]
    MissingInput,

    #[error("reading wasm {0}: {1}")]
    ReadWasm(PathBuf, std::io::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Wasm(#[from] wasm::Error),

    #[error(transparent)]
    SpecTools(#[from] soroban_spec_tools::contract::Error),

    #[error("the WASM has no contractmetav0 custom section")]
    NoMeta,

    #[error("the WASM's contractmetav0 does not record a `bldimg` entry; cannot verify")]
    MissingBldimg,

    #[error("the WASM's contractmetav0 does not record any SEP-58 source-identification entry (source_repo+source_rev, tarball_url, or tarball_sha256); cannot verify")]
    MissingSourceId,

    #[error(
        "the WASM's `{field}` value {value:?} does not match the SEP-58 format regex `{regex}`"
    )]
    MetaFormat {
        field: &'static str,
        value: String,
        regex: &'static str,
    },

    #[error("the WASM records `source_rev` but not `source_repo`; SEP-58 requires both together")]
    SourceRevWithoutRepo,
}

/// SEP-58 metadata extracted from a contract's `contractmetav0` section.
///
/// `cliver` is intentionally not captured: the rebuild container re-injects it,
/// so verify's job is to ensure the rebuild's cliver matches the original's
/// (which it will when `bldimg` resolves to the same container).
#[derive(Debug, Clone)]
pub struct ExtractedMetadata {
    pub bldimg: String,
    pub source_repo: Option<String>,
    pub source_rev: Option<String>,
    pub tarball_url: Option<String>,
    pub tarball_sha256: Option<String>,
    pub bldopts: Vec<String>,
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(false);

        let wasm_bytes = self.fetch_wasm().await?;
        let meta = extract_metadata(&wasm_bytes)?;

        print.infoln(format!("bldimg: {}", meta.bldimg));
        if let Some(v) = &meta.source_repo {
            print.infoln(format!("source_repo: {v}"));
        }
        if let Some(v) = &meta.source_rev {
            print.infoln(format!("source_rev: {v}"));
        }
        if let Some(v) = &meta.tarball_url {
            print.infoln(format!("tarball_url: {v}"));
        }
        if let Some(v) = &meta.tarball_sha256 {
            print.infoln(format!("tarball_sha256: {v}"));
        }
        if !meta.bldopts.is_empty() {
            print.infoln(format!("bldopt entries ({}):", meta.bldopts.len()));
            for o in &meta.bldopts {
                print.blankln(format!("  • {o}"));
            }
        }

        Ok(())
    }

    async fn fetch_wasm(&self) -> Result<Vec<u8>, Error> {
        match (&self.contract_id, &self.wasm) {
            (Some(id), None) => {
                let network = self.network.get(&self.locator)?;
                let resolved =
                    id.resolve_contract_id(&self.locator, &network.network_passphrase)?;
                Ok(wasm::fetch_from_contract(&resolved, &network).await?)
            }
            (None, Some(path)) => std::fs::read(path).map_err(|e| Error::ReadWasm(path.clone(), e)),
            _ => Err(Error::MissingInput),
        }
    }
}

/// Walk the WASM's `contractmetav0` entries and pull out the SEP-58 fields we
/// need to drive a rebuild. Errors when `bldimg` is absent or when no source
/// identification is recorded, since neither has a sensible default.
pub fn extract_metadata(wasm: &[u8]) -> Result<ExtractedMetadata, Error> {
    let spec = Spec::new(wasm)?;
    if spec.meta.is_empty() {
        return Err(Error::NoMeta);
    }

    let mut bldimg: Option<String> = None;
    let mut source_repo: Option<String> = None;
    let mut source_rev: Option<String> = None;
    let mut tarball_url: Option<String> = None;
    let mut tarball_sha256: Option<String> = None;
    let mut bldopts: Vec<String> = Vec::new();

    for entry in &spec.meta {
        let ScMetaEntry::ScMetaV0(ScMetaV0 { key, val }) = entry;
        let k = key.to_string();
        let v = val.to_string();
        match k.as_str() {
            "bldimg" => bldimg = Some(v),
            "source_repo" => source_repo = Some(v),
            "source_rev" => source_rev = Some(v),
            "tarball_url" => tarball_url = Some(v),
            "tarball_sha256" => tarball_sha256 = Some(v),
            "bldopt" => bldopts.push(v),
            _ => {} // cliver and any user --meta are intentionally ignored
        }
    }

    let bldimg = bldimg.ok_or(Error::MissingBldimg)?;
    if !bldimg_regex().is_match(&bldimg) {
        return Err(Error::MetaFormat {
            field: "bldimg",
            value: bldimg,
            regex: BLDIMG_REGEX_STR,
        });
    }

    if let Some(v) = &source_rev {
        if !source_rev_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "source_rev",
                value: v.clone(),
                regex: SOURCE_REV_REGEX_STR,
            });
        }
    }
    if let Some(v) = &source_repo {
        if !source_repo_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "source_repo",
                value: v.clone(),
                regex: SOURCE_REPO_REGEX_STR,
            });
        }
    }
    if let Some(v) = &tarball_url {
        if !tarball_url_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "tarball_url",
                value: v.clone(),
                regex: TARBALL_URL_REGEX_STR,
            });
        }
    }
    if let Some(v) = &tarball_sha256 {
        if !tarball_sha256_regex().is_match(v) {
            return Err(Error::MetaFormat {
                field: "tarball_sha256",
                value: v.clone(),
                regex: TARBALL_SHA256_REGEX_STR,
            });
        }
    }

    // SEP-58 lists `source_repo+source_rev` as a conformant combination. We
    // refuse `source_rev` without `source_repo` here so the user sees a
    // pointed error rather than a downstream "can't clone repo" surprise.
    if source_rev.is_some() && source_repo.is_none() {
        return Err(Error::SourceRevWithoutRepo);
    }

    if source_repo.is_none()
        && source_rev.is_none()
        && tarball_url.is_none()
        && tarball_sha256.is_none()
    {
        return Err(Error::MissingSourceId);
    }

    Ok(ExtractedMetadata {
        bldimg,
        source_repo,
        source_rev,
        tarball_url,
        tarball_sha256,
        bldopts,
    })
}

// These mirror the regex strings used in verifiable.rs. They're kept here only
// so `Error::MetaFormat` can render the regex back to the user as part of the
// error message. The actual matching uses the helpers from verifiable.rs.
const BLDIMG_REGEX_STR: &str =
    r"^(?:localhost(?::\d+)?|[^\s@/]*[.:][^\s@/]*)/[^\s@]+@sha256:[0-9a-f]{64}$";
const SOURCE_REV_REGEX_STR: &str = r"^[0-9a-f]{40}$";
const SOURCE_REPO_REGEX_STR: &str = r"^(https?://\S+|github:[^/\s]+/[^/\s]+)$";
const TARBALL_URL_REGEX_STR: &str = r"^https?://\S+$";
const TARBALL_SHA256_REGEX_STR: &str = r"^[0-9a-f]{64}$";

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use stellar_xdr::curr::{Limited, Limits, ScMetaEntry, ScMetaV0, WriteXdr};

    fn make_wasm_with_meta(entries: &[(&str, &str)]) -> Vec<u8> {
        let xdr = encode_meta(entries);
        let mut wasm = empty_wasm_module();
        wasm_gen::write_custom_section(&mut wasm, "contractmetav0", &xdr);
        wasm
    }

    fn empty_wasm_module() -> Vec<u8> {
        // Minimal valid WASM: magic + version, no sections.
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    fn encode_meta(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut writer = Limited::new(Cursor::new(&mut buf), Limits::none());
        for (k, v) in entries {
            ScMetaEntry::ScMetaV0(ScMetaV0 {
                key: (*k).to_string().try_into().unwrap(),
                val: (*v).to_string().try_into().unwrap(),
            })
            .write_xdr(&mut writer)
            .unwrap();
        }
        buf
    }

    fn good_bldimg() -> String {
        format!("docker.io/stellar/stellar-cli@sha256:{}", "a".repeat(64))
    }

    #[test]
    fn extract_metadata_happy_path_git_source() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", &"b".repeat(40)),
            ("bldopt", "--locked"),
            ("bldopt", "--meta=home_domain=fnando.com"),
            ("home_domain", "fnando.com"),
            ("cliver", "26.0.0#abcdef"),
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        assert_eq!(meta.bldimg, good_bldimg());
        assert_eq!(
            meta.source_repo.as_deref(),
            Some("https://github.com/foo/bar")
        );
        assert_eq!(meta.source_rev.as_deref(), Some("b".repeat(40).as_str()));
        assert_eq!(
            meta.bldopts,
            vec![
                "--locked".to_string(),
                "--meta=home_domain=fnando.com".to_string()
            ]
        );
        assert!(meta.tarball_url.is_none());
        assert!(meta.tarball_sha256.is_none());
    }

    #[test]
    fn extract_metadata_happy_path_tarball_pair() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("tarball_url", "https://example.com/src.tar.gz"),
            ("tarball_sha256", &"f".repeat(64)),
            ("bldopt", "--locked"),
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        assert_eq!(
            meta.tarball_url.as_deref(),
            Some("https://example.com/src.tar.gz")
        );
        assert_eq!(
            meta.tarball_sha256.as_deref(),
            Some("f".repeat(64).as_str())
        );
        assert!(meta.source_repo.is_none());
        assert!(meta.source_rev.is_none());
    }

    #[test]
    fn extract_metadata_missing_bldimg_errors() {
        let wasm = make_wasm_with_meta(&[
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", &"b".repeat(40)),
        ]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::MissingBldimg));
    }

    #[test]
    fn extract_metadata_missing_source_id_errors() {
        let wasm = make_wasm_with_meta(&[("bldimg", &good_bldimg())]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::MissingSourceId));
    }

    #[test]
    fn extract_metadata_source_rev_without_repo_errors() {
        let wasm =
            make_wasm_with_meta(&[("bldimg", &good_bldimg()), ("source_rev", &"b".repeat(40))]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::SourceRevWithoutRepo));
    }

    #[test]
    fn extract_metadata_bad_bldimg_format_errors() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", "stellar/stellar-cli@sha256:abc"), // implicit hub + short
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", &"b".repeat(40)),
        ]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(
            err,
            Error::MetaFormat {
                field: "bldimg",
                ..
            }
        ));
    }

    #[test]
    fn extract_metadata_bad_source_rev_format_errors() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", "not-a-sha"),
        ]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(
            err,
            Error::MetaFormat {
                field: "source_rev",
                ..
            }
        ));
    }

    #[test]
    fn extract_metadata_bad_tarball_sha256_format_errors() {
        let wasm = make_wasm_with_meta(&[("bldimg", &good_bldimg()), ("tarball_sha256", "abc")]);
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(
            err,
            Error::MetaFormat {
                field: "tarball_sha256",
                ..
            }
        ));
    }

    #[test]
    fn extract_metadata_ignores_cliver_and_user_meta() {
        let wasm = make_wasm_with_meta(&[
            ("bldimg", &good_bldimg()),
            ("source_repo", "https://github.com/foo/bar"),
            ("source_rev", &"b".repeat(40)),
            ("cliver", "26.0.0#abcdef"),
            ("home_domain", "fnando.com"),
            ("author", "alice"),
        ]);
        let meta = extract_metadata(&wasm).unwrap();
        // cliver and user meta land in neither bldopts nor source-ids.
        assert!(meta.bldopts.is_empty());
    }

    #[test]
    fn extract_metadata_empty_meta_errors() {
        let wasm = empty_wasm_module(); // no contractmetav0 section
        let err = extract_metadata(&wasm).unwrap_err();
        assert!(matches!(err, Error::NoMeta));
    }
}
