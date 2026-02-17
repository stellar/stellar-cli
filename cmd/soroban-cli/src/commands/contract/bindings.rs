pub mod flutter;
pub mod java;
pub mod php;
pub mod python;
pub mod rust;
pub mod swift;
pub mod typescript;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Generate Rust bindings
    Rust(rust::Cmd),

    /// Generate a TypeScript / JavaScript package
    Typescript(Box<typescript::Cmd>),

    /// Generate Python bindings
    Python(python::Cmd),

    /// Generate Java bindings
    Java(java::Cmd),

    /// Generate Flutter bindings
    Flutter(flutter::Cmd),

    /// Generate Swift bindings
    Swift(swift::Cmd),

    /// Generate PHP bindings
    Php(php::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rust(#[from] rust::Error),

    #[error(transparent)]
    Typescript(#[from] typescript::Error),

    #[error(transparent)]
    Python(#[from] python::Error),

    #[error(transparent)]
    Java(#[from] java::Error),

    #[error(transparent)]
    Flutter(#[from] flutter::Error),

    #[error(transparent)]
    Swift(#[from] swift::Error),

    #[error(transparent)]
    Php(#[from] php::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Rust(rust) => rust.run()?,
            Cmd::Typescript(ts) => ts.run().await?,
            Cmd::Python(python) => python.run()?,
            Cmd::Java(java) => java.run()?,
            Cmd::Flutter(flutter) => flutter.run()?,
            Cmd::Swift(swift) => swift.run()?,
            Cmd::Php(php) => php.run()?,
        }
        Ok(())
    }
}
