use crate::commands::tx::new::manage_buy_offer;

#[derive(clap::Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub args: super::args::Args,
    #[command(flatten)]
    pub op: manage_buy_offer::Cmd,
}
