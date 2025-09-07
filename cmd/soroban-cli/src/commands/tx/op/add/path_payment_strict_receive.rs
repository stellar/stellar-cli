use crate::commands::tx::new::path_payment_strict_receive;

#[derive(clap::Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub args: super::args::Args,
    #[command(flatten)]
    pub op: path_payment_strict_receive::Cmd,
}
