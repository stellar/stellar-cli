pub mod java;
pub mod json;
pub mod python;
pub mod rust;
pub mod typescript;
pub mod mcp_server;

use crate::commands::global;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Generate Json Bindings
    Json(json::Cmd),

    /// Generate Rust bindings
    Rust(rust::Cmd),

    /// Generate a TypeScript / JavaScript package
    Typescript(Box<typescript::Cmd>),

    /// Generate Python bindings
    Python(python::Cmd),

    /// Generate Java bindings
    Java(java::Cmd),

    /// Generate MCP Server bindings
    McpServer(Box<mcp_server::Cmd>),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Json(#[from] json::Error),

    #[error(transparent)]
    Rust(#[from] rust::Error),

    #[error(transparent)]
    Typescript(#[from] typescript::Error),

    #[error(transparent)]
    Python(#[from] python::Error),

    #[error(transparent)]
    Java(#[from] java::Error),

    #[error(transparent)]
    McpServer(#[from] mcp_server::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: Option<&global::Args>) -> Result<(), Error> {
        match &self {
            Cmd::Json(json) => json.run()?,
            Cmd::Rust(rust) => rust.run()?,
            Cmd::Typescript(ts) => ts.run().await?,
            Cmd::Python(python) => python.run()?,
            Cmd::Java(java) => java.run()?,
            Cmd::McpServer(mcp) => mcp.run(global_args).await?,
        }
        Ok(())
    }
}
