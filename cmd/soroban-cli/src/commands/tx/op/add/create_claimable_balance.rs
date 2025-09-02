use crate::commands::tx::new::create_claimable_balance;

#[derive(clap::Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub args: super::args::Args,
    #[command(flatten)]
    pub op: create_claimable_balance::Cmd,
}
