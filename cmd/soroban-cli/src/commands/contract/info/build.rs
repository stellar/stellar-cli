use super::shared::{self, Fetched};
use crate::commands::contract::info::shared::fetch;
use crate::{commands::global, print::Print, utils::http};
use base64::Engine as _;
use clap::Parser;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract;
use soroban_spec_tools::contract::Spec;
use std::fmt::Debug;
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

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

    #[error("'source_repo' meta entry '{0}' has prefix unsupported, only 'github:' supported")]
    SourceRepoUnsupported(String),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("GitHub attestation not found")]
    AttestationNotFound,

    #[error("GitHub attestation invalid")]
    AttestationInvalid,

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
        let Some(github_source_repo) = source_repo.strip_prefix("github:") else {
            return Err(Error::SourceRepoUnsupported(source_repo));
        };

        let url = format!(
            "https://api.github.com/repos/{github_source_repo}/attestations/sha256:{wasm_hash_hex}"
        );
        print.infoln(format!("Collecting GitHub attestation from {url}"));
        let resp = http::client().get(url).send().await?;
        let resp: gh_attest_resp::Root = resp.json().await?;

        // Find the SLSA provenance attestation (not the Release attestation)
        // GitHub may attach multiple attestations, and we need the one with predicate_type
        // matching "https://slsa.dev/provenance/v1"
        let payload = resp
            .attestations
            .iter()
            .find_map(|attestation| {
                let payload = base64::engine::general_purpose::STANDARD
                    .decode(&attestation.bundle.dsse_envelope.payload)
                    .ok()?;
                let payload: gh_payload::Root = serde_json::from_slice(&payload).ok()?;
                if payload.predicate_type == "https://slsa.dev/provenance/v1" {
                    Some(payload)
                } else {
                    None
                }
            })
            .ok_or(Error::AttestationNotFound)?;

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
        pub predicate_type: String,
        pub predicate: Predicate,
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
