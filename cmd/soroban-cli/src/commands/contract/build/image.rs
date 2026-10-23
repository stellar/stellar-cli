use std::{
    collections::{BTreeMap, HashMap},
    fmt::{self, Write},
};

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::commands::global;

const DOCKER_HUB_TAGS_URL: &str =
    "https://hub.docker.com/v2/repositories/stellar/stellar-cli/tags?page_size=100";

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    Multi,
    Amd64,
    Arm64,
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Multi => write!(f, "multi"),
            Self::Amd64 => write!(f, "amd64"),
            Self::Arm64 => write!(f, "arm64"),
        }
    }
}

/// Discover and search official stellar-cli Docker images for reproducible builds
#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(subcommand)]
    pub cmd: Option<SubCmd>,

    /// Format of output: text or json
    #[arg(long, value_enum, default_value_t, global = true)]
    pub output: OutputFormat,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SubCmd {
    /// List available CLI docker images (defaults to latest 20)
    List(ListCmd),
    /// Search available CLI docker images with filters
    Search(SearchCmd),
}

#[derive(Parser, Debug, Clone)]
pub struct ListCmd {
    /// Maximum number of images to list
    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    /// Filter by architecture (multi, amd64, arm64)
    #[arg(long, value_enum)]
    pub arch: Option<Arch>,
}

impl Default for ListCmd {
    fn default() -> Self {
        Self {
            limit: 20,
            arch: None,
        }
    }
}

#[derive(Parser, Debug, Clone, Default)]
pub struct SearchCmd {
    /// Filter by CLI version, commit SHA, or version-commit (e.g. 21.6.0 or 1228cff)
    #[arg(long)]
    pub version: Option<String>,

    /// Filter by architecture (multi, amd64, arm64)
    #[arg(long, value_enum)]
    pub arch: Option<Arch>,

    /// Filter by Rust version (e.g. 1.95.0)
    #[arg(long)]
    pub rust_version: Option<String>,

    /// Filter by release date (YYYY-MM-DD): list images released on or before this date
    #[arg(long)]
    pub date: Option<String>,

    /// Maximum number of images to return
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("network request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("failed to serialize JSON output: {0}")]
    Json(#[from] serde_json::Error),

    #[error("failed to fetch tags from Docker Hub: HTTP {0}")]
    HttpStatus(reqwest::StatusCode),
}

#[derive(Debug, Clone, Deserialize)]
pub struct DockerHubResponse {
    pub count: usize,
    pub next: Option<String>,
    pub results: Vec<DockerHubTag>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DockerHubTag {
    pub name: String,
    pub last_updated: Option<String>,
    pub tag_last_pushed: Option<String>,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTag {
    pub raw_tag: String,
    pub version: String,
    pub commit: Option<String>,
    pub rust_version: Option<String>,
    pub debian: Option<String>,
    pub arch: Arch,
    pub is_alias: bool,
    pub released: String,
    pub released_date: String,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub rust_version: String,
    pub debian: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub released: String,
    pub aliases: Vec<String>,
    pub architectures: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonSearchResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub images: Vec<ImageEntry>,
}

pub fn parse_tag(tag: &DockerHubTag) -> Option<ParsedTag> {
    parse_tag_parts(
        &tag.name,
        tag.tag_last_pushed
            .as_deref()
            .or(tag.last_updated.as_deref()),
        tag.digest.as_deref(),
    )
}

pub fn parse_tag_parts(
    name: &str,
    released_raw: Option<&str>,
    digest: Option<&str>,
) -> Option<ParsedTag> {
    if name == "latest" {
        return None;
    }

    let released = released_raw.unwrap_or("").to_string();
    let released_date = if released.len() >= 10 {
        released[..10].to_string()
    } else {
        released.clone()
    };

    // If tag contains "-rust", it's a toolchain-specific build tag
    if let Some((prefix, suffix)) = name.split_once("-rust") {
        // Parse prefix: {version}-{commit} or just {version}
        let (version, commit) = if let Some((v, c)) = prefix.rsplit_once('-') {
            if c.len() >= 7 && c.chars().all(|ch| ch.is_ascii_hexdigit()) {
                (v.to_string(), Some(c.to_string()))
            } else {
                (prefix.to_string(), None)
            }
        } else {
            (prefix.to_string(), None)
        };

        // Parse suffix: {rust_version}-{debian}[-{arch}][-{revision}]
        // Suffix starts right after "-rust", so e.g. "1.95.0-slim-trixie-amd64-0"
        let mut rest = suffix;

        // Strip revision if present (e.g. "-0", "-1")
        if let Some((before_rev, rev)) = rest.rsplit_once('-') {
            if !rev.is_empty() && rev.chars().all(|ch| ch.is_ascii_digit()) {
                rest = before_rev;
            }
        }

        // Determine architecture
        let (arch, rest_without_arch) = if let Some(stripped) = rest.strip_suffix("-amd64") {
            (Arch::Amd64, stripped)
        } else if let Some(stripped) = rest.strip_suffix("-arm64") {
            (Arch::Arm64, stripped)
        } else {
            (Arch::Multi, rest)
        };

        // Split rust_version and debian
        let (rust_version, debian) = if let Some((r, d)) = rest_without_arch.split_once('-') {
            (Some(r.to_string()), Some(d.to_string()))
        } else {
            (Some(rest_without_arch.to_string()), None)
        };

        Some(ParsedTag {
            raw_tag: name.to_string(),
            version,
            commit,
            rust_version,
            debian,
            arch,
            is_alias: false,
            released,
            released_date,
            digest: digest.map(ToString::to_string),
        })
    } else {
        // Check if name is a clean semver version like 21.6.0 (alias tag)
        let semver_part = name.strip_prefix('v').unwrap_or(name);
        let parts: Vec<&str> = semver_part.split('.').collect();
        if parts.len() == 3 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
            Some(ParsedTag {
                raw_tag: name.to_string(),
                version: semver_part.to_string(),
                commit: None,
                rust_version: None,
                debian: None,
                arch: Arch::Multi,
                is_alias: true,
                released,
                released_date,
                digest: digest.map(ToString::to_string),
            })
        } else {
            None
        }
    }
}

pub struct ImageFilter<'a> {
    pub version: Option<&'a str>,
    pub arch: Option<Arch>,
    pub rust_version: Option<&'a str>,
    pub date: Option<&'a str>,
}

impl ImageFilter<'_> {
    pub fn matches(&self, tag: &ParsedTag) -> bool {
        if let Some(v_filter) = self.version {
            let matched = if let Some(commit) = &tag.commit {
                tag.version == v_filter
                    || commit.starts_with(v_filter)
                    || format!("{}-{}", tag.version, commit).starts_with(v_filter)
            } else {
                tag.version == v_filter || tag.raw_tag.starts_with(v_filter)
            };
            if !matched {
                return false;
            }
        }

        if let Some(arch_filter) = self.arch {
            if tag.arch != arch_filter {
                return false;
            }
        }

        if let Some(rust_filter) = self.rust_version {
            if let Some(rust_ver) = &tag.rust_version {
                if rust_ver != rust_filter {
                    return false;
                }
            } else if !tag.is_alias {
                return false;
            }
        }

        if let Some(date_filter) = self.date {
            if !tag.released_date.is_empty() && tag.released_date.as_str() > date_filter {
                return false;
            }
        }

        true
    }
}

type GroupKey = (String, Option<String>, String, String);
type GroupEntry = (GroupKey, ImageEntry, Option<String>);

/// Group parsed tags into logic image releases
pub fn group_tags(parsed_tags: &[ParsedTag]) -> Vec<ImageEntry> {
    let mut groups: Vec<GroupEntry> = Vec::new();
    let mut aliases: Vec<ParsedTag> = Vec::new();

    for tag in parsed_tags {
        if tag.is_alias {
            aliases.push(tag.clone());
            continue;
        }

        let key = (
            tag.version.clone(),
            tag.commit.clone(),
            tag.rust_version.clone().unwrap_or_default(),
            tag.debian.clone().unwrap_or_default(),
        );

        if let Some((_, entry, _)) = groups.iter_mut().find(|(k, _, _)| k == &key) {
            entry
                .architectures
                .insert(tag.arch.to_string(), tag.raw_tag.clone());
        } else {
            let mut architectures = BTreeMap::new();
            architectures.insert(tag.arch.to_string(), tag.raw_tag.clone());

            let entry = ImageEntry {
                version: Some(tag.version.clone()),
                rust_version: tag.rust_version.clone().unwrap_or_default(),
                debian: tag.debian.clone().unwrap_or_default(),
                commit: tag.commit.clone(),
                released: tag.released.clone(),
                aliases: Vec::new(),
                architectures,
            };
            groups.push((key, entry, tag.digest.clone()));
        }
    }

    // Attach aliases to matching groups by digest or matching version
    for alias in aliases {
        let mut matched = false;
        if let Some(alias_digest) = &alias.digest {
            for (_, entry, digest) in &mut groups {
                if digest.as_ref() == Some(alias_digest) {
                    if !entry.aliases.contains(&alias.raw_tag) {
                        entry.aliases.push(alias.raw_tag.clone());
                    }
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            for (_, entry, _) in &mut groups {
                if entry.version.as_deref() == Some(&alias.version) {
                    if !entry.aliases.contains(&alias.raw_tag) {
                        entry.aliases.push(alias.raw_tag.clone());
                    }
                    break;
                }
            }
        }
    }

    groups.into_iter().map(|(_, entry, _)| entry).collect()
}

/// Format tabulated rows for output
pub fn format_tabulated_output(
    parsed_tags: &[ParsedTag],
    groups: &[ImageEntry],
    filter: &ImageFilter,
    limit: Option<usize>,
) -> String {
    // Map from raw_tag or (version, arch) to metadata for alias row enrichment
    let mut alias_meta: HashMap<String, (&str, &str)> = HashMap::new();
    for g in groups {
        for alias in &g.aliases {
            alias_meta.insert(alias.clone(), (&g.rust_version, &g.debian));
        }
    }

    // Build rows to display
    let mut rows: Vec<(String, String, String, String, String, bool)> = Vec::new();

    for tag in parsed_tags {
        if !filter.matches(tag) {
            continue;
        }

        if tag.is_alias {
            let (rust, debian) = alias_meta.get(&tag.raw_tag).copied().unwrap_or(("", ""));
            // If rust_version filter is set and alias doesn't match, skip
            if let Some(rf) = filter.rust_version {
                if rust != rf {
                    continue;
                }
            }
            rows.push((
                tag.raw_tag.clone(),
                tag.arch.to_string(),
                rust.to_string(),
                debian.to_string(),
                tag.released_date.clone(),
                true,
            ));
        } else {
            rows.push((
                tag.raw_tag.clone(),
                tag.arch.to_string(),
                tag.rust_version.clone().unwrap_or_default(),
                tag.debian.clone().unwrap_or_default(),
                tag.released_date.clone(),
                false,
            ));
        }
    }

    if let Some(l) = limit {
        rows.truncate(l);
    }

    let mut out = String::new();
    let _ = writeln!(out, "TAG                                                                             ARCH   RUST     DEBIAN         RELEASED");

    for (tag, arch, rust, debian, released, is_alias) in rows {
        let alias_str = if is_alias { "  (alias)" } else { "" };
        let _ = writeln!(
            out,
            "{tag:<79} {arch:<6} {rust:<8} {debian:<14} {released}{alias_str}"
        );
    }

    out
}

pub async fn fetch_docker_tags() -> Result<Vec<DockerHubTag>, Error> {
    let client = reqwest::Client::new();
    let mut url = Some(DOCKER_HUB_TAGS_URL.to_string());
    let mut tags = Vec::new();

    while let Some(current_url) = url {
        let response = client
            .get(&current_url)
            .header("User-Agent", "stellar-cli")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::HttpStatus(response.status()));
        }

        let resp_json: DockerHubResponse = response.json().await?;
        tags.extend(resp_json.results);

        // Fetch up to 2 pages (200 tags max) to be responsive and bounded
        if tags.len() >= 200 {
            break;
        }
        url = resp_json.next;
    }

    Ok(tags)
}

fn handle_list(
    list_cmd: &ListCmd,
    output: OutputFormat,
    parsed_tags: &[ParsedTag],
    groups: &[ImageEntry],
) -> Result<(), Error> {
    let filter = ImageFilter {
        version: None,
        arch: list_cmd.arch,
        rust_version: None,
        date: None,
    };

    if output == OutputFormat::Json {
        let mut matched_groups: Vec<ImageEntry> = groups
            .iter()
            .filter(|g| {
                if let Some(arch) = list_cmd.arch {
                    g.architectures.contains_key(&arch.to_string())
                } else {
                    true
                }
            })
            .cloned()
            .collect();
        matched_groups.truncate(list_cmd.limit);

        let result = JsonSearchResult {
            version: None,
            images: matched_groups,
        };
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        let text = format_tabulated_output(parsed_tags, groups, &filter, Some(list_cmd.limit));
        print!("{text}");
    }
    Ok(())
}

fn handle_search(
    search_cmd: &SearchCmd,
    output: OutputFormat,
    parsed_tags: &[ParsedTag],
    groups: &[ImageEntry],
) -> Result<(), Error> {
    let filter = ImageFilter {
        version: search_cmd.version.as_deref(),
        arch: search_cmd.arch,
        rust_version: search_cmd.rust_version.as_deref(),
        date: search_cmd.date.as_deref(),
    };

    if output == OutputFormat::Json {
        let mut matched_groups: Vec<ImageEntry> = groups
            .iter()
            .filter(|g| {
                if let Some(v) = search_cmd.version.as_deref() {
                    let v_match = g.version.as_deref() == Some(v)
                        || g.commit.as_ref().is_some_and(|c| c.starts_with(v))
                        || format!(
                            "{}-{}",
                            g.version.as_deref().unwrap_or_default(),
                            g.commit.as_deref().unwrap_or_default()
                        )
                        .starts_with(v);
                    if !v_match {
                        return false;
                    }
                }
                if let Some(arch) = search_cmd.arch {
                    if !g.architectures.contains_key(&arch.to_string()) {
                        return false;
                    }
                }
                if let Some(rv) = search_cmd.rust_version.as_deref() {
                    if g.rust_version != rv {
                        return false;
                    }
                }
                if let Some(date) = search_cmd.date.as_deref() {
                    let g_date = if g.released.len() >= 10 {
                        &g.released[..10]
                    } else {
                        &g.released
                    };
                    if g_date > date {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        if let Some(l) = search_cmd.limit {
            matched_groups.truncate(l);
        }

        // Remove version field inside each entry when top-level version matches
        let top_version = search_cmd.version.clone();
        if top_version.is_some() {
            for entry in &mut matched_groups {
                entry.version = None;
            }
        }

        let result = JsonSearchResult {
            version: top_version,
            images: matched_groups,
        };
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        let text = format_tabulated_output(parsed_tags, groups, &filter, search_cmd.limit);
        print!("{text}");
    }
    Ok(())
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let raw_tags = fetch_docker_tags().await?;

        let mut parsed_tags: Vec<ParsedTag> = raw_tags.iter().filter_map(parse_tag).collect();

        // Sort by released date newest first
        parsed_tags.sort_by(|a, b| b.released.cmp(&a.released));

        let groups = group_tags(&parsed_tags);

        match &self.cmd {
            Some(SubCmd::List(list_cmd)) => {
                handle_list(list_cmd, self.output, &parsed_tags, &groups)?;
            }
            None => {
                let default_list = ListCmd::default();
                handle_list(&default_list, self.output, &parsed_tags, &groups)?;
            }
            Some(SubCmd::Search(search_cmd)) => {
                handle_search(search_cmd, self.output, &parsed_tags, &groups)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tag_with_commit_and_arch() {
        let raw = "21.6.0-1228cff8022b804659750b94b315932b0e0f3f6a-rust1.95.0-slim-trixie-amd64";
        let parsed = parse_tag_parts(raw, Some("2026-05-20T00:00:00Z"), Some("sha256:1111"))
            .expect("should parse");

        assert_eq!(parsed.raw_tag, raw);
        assert_eq!(parsed.version, "21.6.0");
        assert_eq!(
            parsed.commit.as_deref(),
            Some("1228cff8022b804659750b94b315932b0e0f3f6a")
        );
        assert_eq!(parsed.rust_version.as_deref(), Some("1.95.0"));
        assert_eq!(parsed.debian.as_deref(), Some("slim-trixie"));
        assert_eq!(parsed.arch, Arch::Amd64);
        assert!(!parsed.is_alias);
        assert_eq!(parsed.released_date, "2026-05-20");
    }

    #[test]
    fn test_parse_tag_multi_arch() {
        let raw = "21.6.0-1228cff8022b804659750b94b315932b0e0f3f6a-rust1.95.0-slim-trixie";
        let parsed = parse_tag_parts(raw, Some("2026-05-20T00:00:00Z"), Some("sha256:2222"))
            .expect("should parse");

        assert_eq!(parsed.arch, Arch::Multi);
        assert_eq!(parsed.rust_version.as_deref(), Some("1.95.0"));
        assert_eq!(parsed.debian.as_deref(), Some("slim-trixie"));
        assert!(!parsed.is_alias);
    }

    #[test]
    fn test_parse_tag_without_commit_and_with_revision() {
        let raw = "26.1.0-rust1.95.0-slim-trixie-arm64-0";
        let parsed = parse_tag_parts(raw, Some("2026-08-26T22:11:29Z"), Some("sha256:3333"))
            .expect("should parse");

        assert_eq!(parsed.version, "26.1.0");
        assert_eq!(parsed.commit, None);
        assert_eq!(parsed.rust_version.as_deref(), Some("1.95.0"));
        assert_eq!(parsed.debian.as_deref(), Some("slim-trixie"));
        assert_eq!(parsed.arch, Arch::Arm64);
        assert!(!parsed.is_alias);
    }

    #[test]
    fn test_parse_alias_tag() {
        let raw = "21.6.0";
        let parsed = parse_tag_parts(raw, Some("2026-05-20T00:00:00Z"), Some("sha256:2222"))
            .expect("should parse");

        assert_eq!(parsed.version, "21.6.0");
        assert_eq!(parsed.arch, Arch::Multi);
        assert!(parsed.is_alias);
    }

    #[test]
    fn test_issue_2593_dataset_grouping_and_filtering() {
        let raw_dataset = vec![
            ("21.6.0", "2026-05-20T00:00:00Z", "sha256:m1"),
            (
                "21.6.0-1228cff8022b804659750b94b315932b0e0f3f6a-rust1.95.0-slim-trixie",
                "2026-05-20T00:00:00Z",
                "sha256:m1",
            ),
            (
                "21.6.0-1228cff8022b804659750b94b315932b0e0f3f6a-rust1.95.0-slim-trixie-amd64",
                "2026-05-20T00:00:00Z",
                "sha256:a1",
            ),
            (
                "21.6.0-1228cff8022b804659750b94b315932b0e0f3f6a-rust1.95.0-slim-trixie-arm64",
                "2026-05-20T00:00:00Z",
                "sha256:r1",
            ),
            (
                "21.6.0-1228cff8022b804659750b94b315932b0e0f3f6a-rust1.94.1-slim-trixie",
                "2026-05-18T00:00:00Z",
                "sha256:m2",
            ),
            (
                "21.6.0-1228cff8022b804659750b94b315932b0e0f3f6a-rust1.94.1-slim-trixie-amd64",
                "2026-05-18T00:00:00Z",
                "sha256:a2",
            ),
            (
                "21.6.0-1228cff8022b804659750b94b315932b0e0f3f6a-rust1.94.1-slim-trixie-arm64",
                "2026-05-18T00:00:00Z",
                "sha256:r2",
            ),
            (
                "21.6.0-9a7bf02e3c1d4b5a6f7e8d9c0b1a2f3e4d5c6b7a-rust1.90.0-slim-bookworm",
                "2026-03-02T00:00:00Z",
                "sha256:m3",
            ),
            (
                "21.6.0-9a7bf02e3c1d4b5a6f7e8d9c0b1a2f3e4d5c6b7a-rust1.90.0-slim-bookworm-amd64",
                "2026-03-02T00:00:00Z",
                "sha256:a3",
            ),
            (
                "21.6.0-9a7bf02e3c1d4b5a6f7e8d9c0b1a2f3e4d5c6b7a-rust1.90.0-slim-bookworm-arm64",
                "2026-03-02T00:00:00Z",
                "sha256:r3",
            ),
        ];

        let tags: Vec<ParsedTag> = raw_dataset
            .into_iter()
            .map(|(n, d, dig)| parse_tag_parts(n, Some(d), Some(dig)).unwrap())
            .collect();

        let groups = group_tags(&tags);
        assert_eq!(groups.len(), 3);

        // Group 1: 1.95.0
        assert_eq!(groups[0].rust_version, "1.95.0");
        assert_eq!(groups[0].aliases, vec!["21.6.0"]);
        assert_eq!(groups[0].architectures.len(), 3);

        // Filter by arch: amd64 only
        let filter_amd64 = ImageFilter {
            version: Some("21.6.0"),
            arch: Some(Arch::Amd64),
            rust_version: None,
            date: None,
        };
        let table_amd64 = format_tabulated_output(&tags, &groups, &filter_amd64, None);
        assert!(table_amd64.contains("amd64"));
        assert!(!table_amd64.contains("arm64"));
        assert!(!table_amd64.contains("(alias)")); // alias is multi, so excluded

        // Filter by date: <= 2026-03-02
        let filter_date = ImageFilter {
            version: None,
            arch: None,
            rust_version: None,
            date: Some("2026-03-02"),
        };
        let table_date = format_tabulated_output(&tags, &groups, &filter_date, None);
        assert!(table_date.contains("9a7bf02e3c1d4b5a6f7e8d9c0b1a2f3e4d5c6b7a"));
        assert!(!table_date.contains("1228cff8022b804659750b94b315932b0e0f3f6a"));

        // Filter by rust_version: 1.94.1
        let filter_rust = ImageFilter {
            version: None,
            arch: None,
            rust_version: Some("1.94.1"),
            date: None,
        };
        let table_rust = format_tabulated_output(&tags, &groups, &filter_rust, None);
        assert!(table_rust.contains("1.94.1"));
        assert!(!table_rust.contains("1.95.0"));
        assert!(!table_rust.contains("1.90.0"));
    }
}
