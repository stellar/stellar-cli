use clap::arg;

#[derive(thiserror::Error, Debug)]
pub enum Error {}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// Sign with secret key
    #[arg(
        long,
        conflicts_with = "sign_with_laboratory",
        env = "STELLAR_SIGN_WITH_SECRET"
    )]
    pub sign_with_key: Option<String>,
    /// Sign with labratory
    #[arg(
        long,
        visible_alias = "sign-with-lab",
        conflicts_with = "sign_with_key",
        env = "STELLAR_SIGN_WITH_LABRATORY"
    )]
    pub sign_with_laboratory: bool,

    #[arg(long, conflicts_with = "sign_with_laboratory")]
    /// If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,

    /// If `--sign-with-*` is used this will remove requirement of being prompted
    #[arg(long)]
    pub yes: bool,
}
