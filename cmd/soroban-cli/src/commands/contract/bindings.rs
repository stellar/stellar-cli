pub mod flutter;
pub mod java;
pub mod kmp;
pub mod php;
pub mod python;
pub mod rust;
pub mod swift;
pub mod typescript;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Generate Rust bindings
    Rust(rust::Cmd),

    /// ⚠️ Deprecated, use the JavaScript Stellar SDK instead (https://github.com/stellar/js-stellar-sdk#cli). Generate a TypeScript / JavaScript package
    Typescript(Box<typescript::Cmd>),

    /// Generate Python bindings (requires external plugin)
    Python(python::Cmd),

    /// Generate Java bindings (requires external plugin)
    Java(java::Cmd),

    /// Generate Flutter bindings (requires external plugin)
    Flutter(flutter::Cmd),

    /// Generate Swift bindings (requires external plugin)
    Swift(swift::Cmd),

    /// Generate PHP bindings (requires external plugin)
    Php(php::Cmd),

    /// Generate Kotlin Multiplatform bindings (requires external plugin)
    Kmp(kmp::Cmd),
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

    #[error(transparent)]
    Kmp(#[from] kmp::Error),
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
            Cmd::Kmp(kmp) => kmp.run()?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Subcommand;

    #[test]
    fn unimplemented_bindings_note_external_plugin() {
        // Rust and TypeScript are the only generators implemented in-CLI; every
        // other binding language is a placeholder that defers to the external
        // plugin, so its help must say so. New placeholders inherit this check.
        const IMPLEMENTED: &[&str] = &["rust", "typescript"];
        let cmd = Cmd::augment_subcommands(clap::Command::new("bindings"));
        for sub in cmd.get_subcommands() {
            let name = sub.get_name();
            if IMPLEMENTED.contains(&name) {
                continue;
            }
            let about = sub.get_about().map(ToString::to_string).unwrap_or_default();
            assert!(
                about.contains("(requires external plugin)"),
                "binding `{name}` is not implemented in-CLI; its help must note `(requires external plugin)`"
            );
        }
    }
}
