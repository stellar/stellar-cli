use super::shared::{self, Fetched};
use crate::commands::contract::info::shared::fetch;
use crate::{commands::global, print::Print, utils::http};
use base64::Engine as _;
use clap::Parser;
use regex::Regex;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;
use std::fmt::Debug;
use stellar_xdr::{ScMetaEntry, ScMetaV0};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Wasm(#[from] shared::Error),

    #[error(transparent)]
    Spec(#[from] contract::Error),

    #[error("'source_repo' meta entry is not stored in the contract")]
    SourceRepoNotSpecified,

    #[error("'source_repo' meta entry '{0}' is not a valid 'github:<owner>/<repo>' reference")]
    SourceRepoInvalid(String),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("GitHub attestation not found")]
    AttestationNotFound,

    #[error("GitHub attestation invalid")]
    AttestationInvalid,

    #[error("GitHub attestation does not attest to this wasm (subject digest does not match wasm hash {0})")]
    AttestationSubjectMismatch(String),

    #[error("GitHub attestation source '{attested}' does not match the looked-up repository '{expected}'")]
    AttestationRepoMismatch { expected: String, attested: String },

    #[error("Stellar asset contract doesn't contain meta information")]
    NoSACMeta(),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        print.warnln("\x1b[31mThis command displays information about the GitHub Actions run that attested to have built the wasm, and does not verify the source code. Please review the run, its workflow, and source code.\x1b[0m".to_string());

        let Fetched { contract, .. } = fetch(&self.common, &print).await?;

        let bytes = match contract {
            shared::Contract::Wasm { wasm_bytes } => wasm_bytes,
            shared::Contract::StellarAssetContract => return Err(Error::NoSACMeta()),
        };

        let wasm_hash = Sha256::digest(&bytes);
        let wasm_hash_hex = hex::encode(wasm_hash);
        print.infoln(format!("Wasm Hash: {wasm_hash_hex}"));

        let spec = Spec::new(&bytes)?;
        let Some(source_repo) = spec.meta.iter().find_map(|meta_entry| {
            let ScMetaEntry::ScMetaV0(ScMetaV0 { key, val }) = meta_entry;
            if key.to_string() == "source_repo" {
                Some(val.to_string())
            } else {
                None
            }
        }) else {
            return Err(Error::SourceRepoNotSpecified);
        };
        print.infoln(format!("Source Repo: {source_repo}"));
        let github_source_repo = extract_github_repo(&source_repo)?;

        let url = format!(
            "https://api.github.com/repos/{github_source_repo}/attestations/sha256:{wasm_hash_hex}"
        );
        print.infoln(format!("Collecting GitHub attestation from {url}"));
        let resp = http::client().get(url).send().await?;
        let resp: gh_attest_resp::Root = resp.json().await?;

        let payload = verified_provenance(&resp, &wasm_hash_hex, github_source_repo)?;

        print.checkln("Attestation found linked to GitHub Actions Workflow Run:");

        let workflow_repo = payload
            .predicate
            .build_definition
            .external_parameters
            .workflow
            .repository;
        let workflow_ref = payload
            .predicate
            .build_definition
            .external_parameters
            .workflow
            .ref_field;
        let workflow_path = payload
            .predicate
            .build_definition
            .external_parameters
            .workflow
            .path;
        let git_commit = &payload
            .predicate
            .build_definition
            .resolved_dependencies
            .first()
            .ok_or(Error::AttestationInvalid)?
            .digest
            .git_commit;
        let runner_environment = payload
            .predicate
            .build_definition
            .internal_parameters
            .github
            .runner_environment
            .as_str();
        print.blankln(format!(" \x1b[34mRepository:\x1b[0m {workflow_repo}"));
        print.blankln(format!(" \x1b[34mRef:\x1b[0m        {workflow_ref}"));
        print.blankln(format!(" \x1b[34mPath:\x1b[0m       {workflow_path}"));
        print.blankln(format!(" \x1b[34mGit Commit:\x1b[0m {git_commit}"));
        match runner_environment
        {
            runner @ "github-hosted" => print.blankln(format!(" \x1b[34mRunner:\x1b[0m     {runner}")),
            runner => print.warnln(format!(" \x1b[34mRunner:\x1b[0m     {runner} (runners not hosted by GitHub could have any configuration or environmental changes)")),
        }
        print.blankln(format!(
            " \x1b[34mRun:\x1b[0m        {}",
            payload.predicate.run_details.metadata.invocation_id
        ));
        print.globeln(format!(
            "View the workflow at {workflow_repo}/blob/{git_commit}/{workflow_path}"
        ));
        print.globeln(format!(
            "View the repo at {workflow_repo}/tree/{git_commit}"
        ));

        Ok(())
    }
}

/// Regex for a SEP-0055 `source_repo` entry, capturing the `<owner>/<repo>`
/// portion. Matching the whole value (prefix included) is what keeps the
/// captured repo free of extra path segments or query/fragment characters that
/// could otherwise inject into the attestation URL.
const GITHUB_SOURCE_REPO_PATTERN: &str = r"^github:([A-Za-z0-9-]+/[A-Za-z0-9._-]+)$";

/// Strictly validate a `source_repo` meta entry, returning its `<owner>/<repo>`
/// portion when it matches SEP-0055's `github:<owner>/<repo>` form.
fn extract_github_repo(source_repo: &str) -> Result<&str, Error> {
    let re = Regex::new(GITHUB_SOURCE_REPO_PATTERN).expect("valid regex");
    let Some(captures) = re.captures(source_repo) else {
        return Err(Error::SourceRepoInvalid(source_repo.to_string()));
    };
    Ok(captures
        .get(1)
        .expect("capture group 1 is present on match")
        .as_str())
}

/// Normalize a SLSA `resolvedDependencies[].uri` to a plain repository URL by
/// dropping the `git+` scheme prefix, the `@<ref>` suffix, and any `.git`
/// suffix, so it can be compared against `https://github.com/<owner>/<repo>`.
fn resolved_dependency_repo_url(uri: &str) -> &str {
    uri.strip_prefix("git+")
        .unwrap_or(uri)
        .split('@')
        .next()
        .unwrap_or("")
        .trim_end_matches(".git")
}

/// Select the SLSA provenance attestation from a GitHub attestations response
/// that is bound to the given wasm and repository.
///
/// GitHub may attach multiple attestations (e.g. a Release attestation, or
/// several provenance records), so every `https://slsa.dev/provenance/v1`
/// payload is considered and the first one satisfying both bindings is
/// returned. Per SEP-0055 the bindings are: the in-toto subject digest must
/// match the local wasm hash (step 7), and the first resolved dependency URI
/// must be the looked-up repository (step 8). When none match, the most
/// specific reason is reported.
fn verified_provenance(
    resp: &gh_attest_resp::Root,
    wasm_hash_hex: &str,
    github_source_repo: &str,
) -> Result<gh_payload::Root, Error> {
    let candidates: Vec<gh_payload::Root> = resp
        .attestations
        .iter()
        .filter_map(|attestation| {
            let payload = base64::engine::general_purpose::STANDARD
                .decode(&attestation.bundle.dsse_envelope.payload)
                .ok()?;
            let payload: gh_payload::Root = serde_json::from_slice(&payload).ok()?;

            (payload.predicate_type == "https://slsa.dev/provenance/v1").then_some(payload)
        })
        .collect();

    if candidates.is_empty() {
        return Err(Error::AttestationNotFound);
    }

    let expected_repo_url = format!("https://github.com/{github_source_repo}");
    let mut saw_subject_match = false;
    let mut attested_repo = None;

    for payload in candidates {
        // Bind to the wasm being inspected (SEP-0055 step 7): the in-toto
        // subject digest must match the locally computed hash.
        let subject_matches = payload
            .subject
            .iter()
            .any(|subject| subject.digest.sha256.eq_ignore_ascii_case(wasm_hash_hex));

        // Bind to the looked-up repository (SEP-0055 step 8): the first
        // resolved dependency must be the repo named in `source_repo`.
        let source_repo_url = payload
            .predicate
            .build_definition
            .resolved_dependencies
            .first()
            .map(|dependency| resolved_dependency_repo_url(&dependency.uri).to_string());
        // GitHub owner/repo names are case-insensitive, so compare accordingly.
        let repo_matches = source_repo_url
            .as_deref()
            .is_some_and(|url| url.eq_ignore_ascii_case(&expected_repo_url));

        if subject_matches && repo_matches {
            return Ok(payload);
        }
        if subject_matches && attested_repo.is_none() {
            saw_subject_match = true;
            attested_repo = Some(source_repo_url.unwrap_or_default());
        }
    }

    // No attestation satisfied both bindings; report the closest failure.
    if let Some(attested) = attested_repo.filter(|_| saw_subject_match) {
        Err(Error::AttestationRepoMismatch {
            expected: expected_repo_url,
            attested,
        })
    } else {
        Err(Error::AttestationSubjectMismatch(wasm_hash_hex.to_string()))
    }
}

mod gh_attest_resp {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        pub attestations: Vec<Attestation>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Attestation {
        pub bundle: Bundle,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Bundle {
        pub dsse_envelope: DsseEnvelope,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DsseEnvelope {
        pub payload: String,
    }
}

mod gh_payload {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        #[serde(default)]
        pub subject: Vec<Subject>,
        pub predicate_type: String,
        pub predicate: Predicate,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Subject {
        pub digest: SubjectDigest,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SubjectDigest {
        #[serde(default)]
        pub sha256: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Predicate {
        pub build_definition: BuildDefinition,
        pub run_details: RunDetails,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BuildDefinition {
        pub external_parameters: ExternalParameters,
        pub internal_parameters: InternalParameters,
        pub resolved_dependencies: Vec<ResolvedDependency>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ExternalParameters {
        pub workflow: Workflow,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Workflow {
        #[serde(rename = "ref")]
        pub ref_field: String,
        pub repository: String,
        pub path: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct InternalParameters {
        pub github: Github,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Github {
        #[serde(rename = "runner_environment")]
        pub runner_environment: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ResolvedDependency {
        pub uri: String,
        pub digest: Digest,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Digest {
        pub git_commit: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RunDetails {
        pub metadata: Metadata,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Metadata {
        pub invocation_id: String,
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_github_repo, gh_attest_resp, verified_provenance, Error};
    use base64::Engine as _;

    const WASM_HASH: &str = "d3e0f12ef8e25358d366c40faf5d4792749c6651d91256c3d888bcdb817829e0";

    /// Build an attestations response whose SLSA provenance payload names the
    /// given subject digest, workflow repository, and resolved dependency URI.
    fn attest_resp_full(
        subject_sha256: &str,
        workflow_repository: &str,
        dependency_uri: &str,
    ) -> gh_attest_resp::Root {
        let payload = serde_json::json!({
            "predicateType": "https://slsa.dev/provenance/v1",
            "subject": [{ "digest": { "sha256": subject_sha256 } }],
            "predicate": {
                "buildDefinition": {
                    "externalParameters": {
                        "workflow": {
                            "ref": "refs/tags/v1.0.0",
                            "repository": workflow_repository,
                            "path": ".github/workflows/release.yml",
                        }
                    },
                    "internalParameters": { "github": { "runner_environment": "github-hosted" } },
                    "resolvedDependencies": [{
                        "uri": dependency_uri,
                        "digest": { "gitCommit": "abcdef0123456789" },
                    }],
                },
                "runDetails": { "metadata": { "invocationId": "https://github.com/x/actions/runs/1" } },
            },
        });
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&payload).unwrap());
        serde_json::from_value(serde_json::json!({
            "attestations": [{ "bundle": { "dsseEnvelope": { "payload": encoded } } }],
        }))
        .unwrap()
    }

    /// Build an attestations response for the common case where the workflow
    /// repository and the resolved dependency both point at `repository`.
    fn attest_resp(subject_sha256: &str, repository: &str) -> gh_attest_resp::Root {
        attest_resp_full(subject_sha256, repository, &format!("git+{repository}"))
    }

    /// Merge several single-attestation responses into one.
    fn combine(responses: Vec<gh_attest_resp::Root>) -> gh_attest_resp::Root {
        gh_attest_resp::Root {
            attestations: responses
                .into_iter()
                .flat_map(|response| response.attestations)
                .collect(),
        }
    }

    #[test]
    fn accepts_attestation_bound_to_wasm_and_repo() {
        let resp = attest_resp(WASM_HASH, "https://github.com/stellar/stellar-cli");
        assert!(verified_provenance(&resp, WASM_HASH, "stellar/stellar-cli").is_ok());
    }

    #[test]
    fn rejects_attestation_for_a_different_wasm() {
        // The subject digest belongs to some other, legitimately attested artifact.
        let other = "1111111111111111111111111111111111111111111111111111111111111111";
        let resp = attest_resp(other, "https://github.com/stellar/stellar-cli");
        assert!(matches!(
            verified_provenance(&resp, WASM_HASH, "stellar/stellar-cli"),
            Err(Error::AttestationSubjectMismatch(_))
        ));
    }

    #[test]
    fn rejects_attestation_from_a_different_repo() {
        let resp = attest_resp(WASM_HASH, "https://github.com/attacker/evil");
        assert!(matches!(
            verified_provenance(&resp, WASM_HASH, "stellar/stellar-cli"),
            Err(Error::AttestationRepoMismatch { .. })
        ));
    }

    #[test]
    fn binds_on_resolved_dependency_uri_not_workflow_repository() {
        // The workflow repository claims the looked-up repo, but the resolved
        // source dependency is a different repo: the binding must reject it.
        let resp = attest_resp_full(
            WASM_HASH,
            "https://github.com/stellar/stellar-cli",
            "git+https://github.com/attacker/evil@refs/heads/main",
        );
        assert!(matches!(
            verified_provenance(&resp, WASM_HASH, "stellar/stellar-cli"),
            Err(Error::AttestationRepoMismatch { .. })
        ));
    }

    #[test]
    fn accepts_resolved_dependency_uri_with_scheme_and_ref() {
        // Matching is on the resolved dependency, so a `git+` scheme and `@<ref>`
        // suffix are tolerated even when the workflow repository differs.
        let resp = attest_resp_full(
            WASM_HASH,
            "https://github.com/whatever/mismatch",
            "git+https://github.com/stellar/stellar-cli@refs/tags/v1.0.0",
        );
        assert!(verified_provenance(&resp, WASM_HASH, "stellar/stellar-cli").is_ok());
    }

    #[test]
    fn selects_matching_attestation_among_several() {
        let other = "1111111111111111111111111111111111111111111111111111111111111111";
        let resp = combine(vec![
            attest_resp(other, "https://github.com/stellar/stellar-cli"),
            attest_resp(WASM_HASH, "https://github.com/stellar/stellar-cli"),
        ]);
        assert!(verified_provenance(&resp, WASM_HASH, "stellar/stellar-cli").is_ok());
    }

    #[test]
    fn accepts_repo_differing_only_in_case() {
        // GitHub owner/repo names are case-insensitive, so an attestation whose
        // resolved dependency differs from `source_repo` only in casing still
        // describes the same repository and must be accepted.
        let resp = attest_resp(WASM_HASH, "https://github.com/Stellar/Stellar-CLI");
        assert!(verified_provenance(&resp, WASM_HASH, "stellar/stellar-cli").is_ok());
    }

    #[test]
    fn accepts_plain_owner_repo() {
        assert_eq!(
            extract_github_repo("github:stellar/stellar-cli").unwrap(),
            "stellar/stellar-cli"
        );
        assert_eq!(
            extract_github_repo("github:Org-1/repo_name.rs").unwrap(),
            "Org-1/repo_name.rs"
        );
    }

    #[test]
    fn rejects_url_injection_via_fragment_or_query() {
        for repo in [
            "github:stellar/legit-repo/attestations/sha256:1111#",
            "github:stellar/legit-repo/attestations/sha256:1111?x=",
            "github:stellar/legit-repo?per_page=30&after=",
            "github:stellar/legit-repo#",
        ] {
            assert!(
                matches!(extract_github_repo(repo), Err(Error::SourceRepoInvalid(_))),
                "expected {repo} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_extra_path_segments() {
        assert!(matches!(
            extract_github_repo("github:stellar/legit-repo/extra"),
            Err(Error::SourceRepoInvalid(_))
        ));
    }

    #[test]
    fn rejects_non_github_prefix() {
        for repo in ["gitlab:owner/repo", "owner/repo", "github:owner", ""] {
            assert!(
                matches!(extract_github_repo(repo), Err(Error::SourceRepoInvalid(_))),
                "expected {repo} to be rejected"
            );
        }
    }
}
