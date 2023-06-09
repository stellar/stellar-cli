pub mod json;
pub mod rust;
pub mod typescript;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Generate Json Bindings
    Json(json::Cmd),

    /// Generate Rust bindings
    Rust(rust::Cmd),

    /// Generate a TypeScript / JavaScript package
    Typescript(typescript::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Json(#[from] json::Error),

    #[error(transparent)]
    Rust(#[from] rust::Error),

    #[error(transparent)]
    Typescript(#[from] typescript::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Json(json) => json.run()?,
            Cmd::Rust(rust) => rust.run()?,
            Cmd::Typescript(ts) => ts.run()?,
        }
        Ok(())
    }
}
