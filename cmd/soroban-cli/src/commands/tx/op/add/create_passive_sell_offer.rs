use crate::commands::tx::new::create_passive_sell_offer;

#[derive(clap::Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub args: super::args::Args,
    #[command(flatten)]
    pub op: create_passive_sell_offer::Cmd,
}
