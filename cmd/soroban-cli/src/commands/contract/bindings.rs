pub mod json;
pub mod rust;
pub mod ts;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Generate Json Bindings
    Json(json::Cmd),

    /// Generate Rust bindings
    Rust(rust::Cmd),
    /// Generate Ts project
    Ts(ts::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Json(#[from] json::Error),

    #[error(transparent)]
    Rust(#[from] rust::Error),

    #[error(transparent)]
    Ts(#[from] ts::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Json(json) => json.run()?,
            Cmd::Rust(rust) => rust.run()?,
            Cmd::Ts(ts) => ts.run()?,
        }
        Ok(())
    }
}
