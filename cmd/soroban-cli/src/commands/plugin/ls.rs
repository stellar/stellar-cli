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
            println!("No Plugins installed. E.g. stellar-hello");
        } else {
            println!("Installed Plugins:\n    {}", plugins.join("\n    "));
        }

        Ok(())
    }
}
