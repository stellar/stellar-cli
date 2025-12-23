use crate::commands::global;

pub mod sign;
pub mod verify;

/// The prefix used for SEP-53 message signing.
/// See: https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0053.md
pub const SEP53_PREFIX: &str = "Stellar Signed Message:\n";

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Sign an arbitrary message using SEP-53
    ///
    /// Signs a message following the SEP-53 specification for arbitrary message signing.
    /// The message is prefixed with "Stellar Signed Message:\n", hashed with SHA-256,
    /// and signed with the ed25519 private key.
    ///
    /// Example: stellar message sign "Hello, World!" --sign-with-key alice
    Sign(sign::Cmd),

    /// Verify a SEP-53 signed message
    ///
    /// Verifies that a signature was produced by the holder of the private key
    /// corresponding to the given public key, following the SEP-53 specification.
    ///
    /// Example: stellar message verify "Hello, World!" --signature <BASE64_SIG> --public-key GABC...
    Verify(verify::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Sign(#[from] sign::Error),

    #[error(transparent)]
    Verify(#[from] verify::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Sign(cmd) => cmd.run(global_args).await?,
            Cmd::Verify(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
