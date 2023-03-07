#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]
pub mod commands;
pub mod network;
pub mod rpc;
pub mod strval;
pub mod toid;
pub mod utils;
pub mod wasm;

pub use commands::Root;

pub fn parse_cmd<T>(s: &str) -> Result<T, clap::Error>
where
    T: clap::CommandFactory + clap::FromArgMatches,
{
    let input = shlex::split(s).ok_or_else(|| {
        clap::Error::raw(
            clap::ErrorKind::InvalidValue,
            format!("Invalid input for command:\n{s}"),
        )
    })?;
    T::from_arg_matches_mut(&mut T::command().no_binary_name(true).get_matches_from(input))
}

pub trait CommandParser<T> {
    fn parse(s: &str) -> Result<T, clap::Error>;
}

impl<T> CommandParser<T> for T
where
    T: clap::CommandFactory + clap::FromArgMatches,
{
    fn parse(s: &str) -> Result<T, clap::Error> {
        parse_cmd(s)
    }
}
