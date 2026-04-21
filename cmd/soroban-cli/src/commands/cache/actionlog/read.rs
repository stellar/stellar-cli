use std::io;

use crate::config::{data, locator};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error("failed to find cache entry {0}")]
    NotFound(String),
    #[error("invalid cache entry ID \"{0}\": expected a ULID")]
    InvalidId(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// ID of the cache entry
    #[arg(long)]
    pub id: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let id: ulid::Ulid = self
            .id
            .parse()
            .map_err(|_| Error::InvalidId(self.id.clone()))?;
        let file = data::actions_dir()?
            .join(id.to_string())
            .with_extension("json");
        tracing::debug!("reading file {}", file.display());
        let mut f = std::fs::File::open(&file).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                Error::NotFound(self.id.clone())
            } else {
                Error::Io(e)
            }
        })?;
        io::copy(&mut f, &mut io::stdout())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;
    use serial_test::serial;

    #[test]
    #[serial]
    fn path_traversal_via_dotdot_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("STELLAR_DATA_HOME", tmp.path());

        let outside = tmp.path().join("outside.json");
        std::fs::write(&outside, r#"{"leaked":true}"#).unwrap();

        let cmd = Cmd {
            id: "../outside".to_string(),
        };

        let err = cmd.run().expect_err("expected error for path-traversal ID");
        assert!(
            matches!(err, Error::InvalidId(_)),
            "expected InvalidId, got {err:?}"
        );
    }

    #[test]
    #[serial]
    fn absolute_path_id_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = EnvGuard::set("STELLAR_DATA_HOME", tmp.path());

        let outside = tmp.path().join("outside.json");
        std::fs::write(&outside, r#"{"leaked":true}"#).unwrap();

        let abs_id = outside
            .to_str()
            .unwrap()
            .trim_end_matches(".json")
            .to_string();
        let cmd = Cmd { id: abs_id };

        let err = cmd.run().expect_err("expected error for absolute-path ID");
        assert!(
            matches!(err, Error::InvalidId(_)),
            "expected InvalidId, got {err:?}"
        );
    }
}
