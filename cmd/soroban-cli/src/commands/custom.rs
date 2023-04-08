use std::process::Command;
use which::which;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Plugin not provided. Should be `soroban plugin` for a binary `soroban-plugin`")]
    MissingSubcommand,
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("Cannot find executable: {0}")]
    ExecutableNotFound(String),
}

pub fn run_external() -> Result<(), Error> {
    let (name, args) = {
        let mut args = std::env::args().skip(1);
        let name = args.next().ok_or(Error::MissingSubcommand)?;
        (name, args)
    };
    let bin = which(format!("soroban-{name}")).map_err(|_| Error::ExecutableNotFound(name))?;
    std::process::exit(
        Command::new(bin)
            .args(args)
            .spawn()?
            .wait()?
            .code()
            .unwrap(),
    );
}
