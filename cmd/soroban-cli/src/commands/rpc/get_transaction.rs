use crate::{
    commands::global,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {}

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        println!("running rpc getTransaction");
        Ok(())
    }
}