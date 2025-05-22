use clap::{command, Parser};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {}

#[derive(thiserror::Error, Debug)]
pub enum Error {}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        println!("fetching tx envelope");
        Ok(())
    }
}