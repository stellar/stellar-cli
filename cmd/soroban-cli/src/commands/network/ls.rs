use super::locator;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,
    /// Get more info about the networks
    #[arg(long, short = 'l')]
    pub long: bool,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let sep = if self.long { "\n\n" } else { "\n" };
        let res = if self.long { self.ls_l() } else { self.ls() }?.join(sep);
        println!("{res}");
        Ok(())
    }

    pub fn ls(&self) -> Result<Vec<String>, Error> {
        Ok(self.config_locator.list_networks()?)
    }

    pub fn ls_l(&self) -> Result<Vec<String>, Error> {
        Ok(self
            .config_locator
            .list_networks_long()?
            .iter()
            .map(|(name, network, _)| {
                let headers = if network.rpc_headers.is_empty() {
                    " not set".to_string()
                } else {
                    let lines: Vec<String> = network
                        .rpc_headers
                        .iter()
                        .map(|(k, _)| format!("  {k}: <concealed>"))
                        .collect();
                    format!("\n{}", lines.join("\n"))
                };

                format!(
                    "Name: {name}\nRPC url: {rpc_url}\nRPC headers:{headers}\nNetwork passphrase: {passphrase}",
                    rpc_url = crate::utils::url::redact_url(&network.rpc_url),
                    passphrase = network.network_passphrase,
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::network::Network;
    use crate::test_utils::{with_cwd_guard, with_env_guard};
    use serial_test::serial;

    #[test]
    #[serial]
    fn ls_l_redacts_rpc_url_password() {
        let tmp = tempfile::tempdir().unwrap();

        with_env_guard(&["STELLAR_CONFIG_HOME", "XDG_CONFIG_HOME"], || {
            with_cwd_guard(|| {
                let global_cfg = tmp.path().join("global");
                std::fs::create_dir_all(&global_cfg).unwrap();
                std::env::set_var("STELLAR_CONFIG_HOME", &global_cfg);

                let work = tmp.path().join("work");
                std::fs::create_dir_all(&work).unwrap();
                std::env::set_current_dir(&work).unwrap();

                let cmd = Cmd {
                    config_locator: locator::Args { config_dir: None },
                    long: true,
                };

                let network = Network {
                    rpc_url: "https://alice:supersecret@rpc.example.com/soroban".to_string(),
                    rpc_headers: Vec::new(),
                    network_passphrase: "Test SDF Network ; September 2015".to_string(),
                };
                cmd.config_locator.write_network("corp", &network).unwrap();

                let rendered = cmd.ls_l().unwrap().join("\n\n");

                assert!(
                    !rendered.contains("supersecret"),
                    "password leaked into `network ls -l` output: {rendered}"
                );
                assert!(
                    rendered.contains("alice:redacted"),
                    "expected `alice:redacted` in `network ls -l` output: {rendered}"
                );
                assert!(
                    rendered.contains("rpc.example.com/soroban"),
                    "expected host and path preserved: {rendered}"
                );
            });
        });
    }
}
