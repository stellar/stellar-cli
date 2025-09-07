use super::default;

#[derive(thiserror::Error, Debug)]
pub enum Error {}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd;

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let plugins = default::list().unwrap_or_default();

        if plugins.is_empty() {
            println!("No plugins installed.");
            println!();
            println!("Plugins are commands available on the path");
            println!("that start with 'stellar-'. E.g. stellar-hello");
            println!();
            println!("https://developers.stellar.org/docs/tools/cli/plugins");
        } else {
            println!("Installed Plugins:\n    {}", plugins.join("\n    "));
        }

        Ok(())
    }
}
