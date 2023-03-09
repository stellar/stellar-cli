use super::locator;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("No such network {name}")]
    MissingNetwork { name: String },
    #[error("Error deleting {path}")]
    DeletingIdFile { path: String },
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// default name
    pub default_name: String,

    #[clap(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let path = self
            .config_locator
            .network_path(&self.default_name)
            .map_err(|_| Error::MissingNetwork {
                name: self.default_name.clone(),
            })?;
        std::fs::remove_file(&path).map_err(|_| Error::DeletingIdFile {
            path: format!("{}", path.display()),
        })
    }
}
