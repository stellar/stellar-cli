#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]

use std::fs;

pub mod error;
mod policy;
pub mod templates;
mod types;

pub use policy::PolicyType;
pub use types::{MethodConfig, PolicyConfig};

pub fn generate_policy(policy_type: PolicyType, params: serde_json::Value) -> Result<String, error::Error> {
    policy::generate_policy(policy_type, &params)
}

pub fn write_policy_to_file(policy: &str, out_dir: &str) -> Result<(), error::Error> {
    fs::create_dir_all(out_dir).map_err(error::Error::Io)?;
    fs::write(format!("{}/policy_contract.rs", out_dir), policy).map_err(error::Error::Io)?;
    Ok(())
}