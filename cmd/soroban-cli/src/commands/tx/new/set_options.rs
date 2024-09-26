use clap::{command, Parser};

use soroban_sdk::xdr::{self};

use crate::{
    commands::{global, tx},
    config::signer_key,
    tx::builder,
};

#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,
    #[arg(long)]
    /// Account of the inflation destination.
    pub inflation_dest: Option<builder::AccountId>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the weight of the master key. If the weight of the master key is updated to 0, it is effectively disabled.
    pub master_weight: Option<u8>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have [a low threshold](../../encyclopedia/security/signatures-multisig.mdx).
    pub low_threshold: Option<u8>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have [a medium threshold](../../encyclopedia/security/signatures-multisig.mdx).
    pub med_threshold: Option<u8>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have [a high threshold](../../encyclopedia/security/signatures-multisig.mdx).
    pub high_threshold: Option<u8>,
    #[arg(long)]
    /// Sets the home domain of an account. See [Federation](../../encyclopedia/network-configuration/federation.mdx).
    pub home_domain: Option<xdr::StringM<32>>,
    #[arg(long, requires = "signer_weight")]
    /// Add, update, or remove a signer from an account.
    pub signer: Option<signer_key::SignerKey>,
    #[arg(long = "signer-weight", requires = "signer")]
    /// Signer weight is a number from 0-255 (inclusive). The signer is deleted if the weight is 0.
    pub signer_weight: Option<u8>,
    #[arg(long, conflicts_with = "clear_required")]
    /// When enabled, an issuer must approve an account before that account can hold its asset.
    /// [More info](https://developers.stellar.org/docs/tokens/control-asset-access#authorization-required-0x1)
    pub set_required: bool,
    #[arg(long, conflicts_with = "clear_revocable")]
    /// When enabled, an issuer can revoke an existing trustline’s authorization, thereby freezing the asset held by an account.
    /// [More info](https://developers.stellar.org/docs/tokens/control-asset-access#authorization-revocable-0x2)
    pub set_revocable: bool,
    #[arg(long, conflicts_with = "clear_clawback_enabled")]
    /// Enables the issuing account to take back (burning) all of the asset.
    /// [More info](https://developers.stellar.org/docs/tokens/control-asset-access#clawback-enabled-0x8)
    pub set_clawback_enabled: bool,
    #[arg(long, conflicts_with = "clear_immutable")]
    /// With this setting, none of the other authorization flags (`AUTH_REQUIRED_FLAG`, `AUTH_REVOCABLE_FLAG`) can be set, and the issuing account can’t be merged.
    /// [More info](https://developers.stellar.org/docs/tokens/control-asset-access#authorization-immutable-0x4)
    pub set_immutable: bool,
    #[arg(long)]
    pub clear_required: bool,
    #[arg(long)]
    pub clear_revocable: bool,
    #[arg(long)]
    pub clear_immutable: bool,
    #[arg(long)]
    pub clear_clawback_enabled: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tx(#[from] tx::args::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        self.tx.handle_and_print(self, global_args).await?;
        Ok(())
    }
}

impl builder::Operation for Cmd {
    fn build_body(&self) -> stellar_xdr::curr::OperationBody {
        let mut set_flags = 0u32;
        let mut set_flag = |flag: xdr::AccountFlags| set_flags |= flag as u32;

        if self.set_required {
            set_flag(xdr::AccountFlags::RequiredFlag);
        };
        if self.set_revocable {
            set_flag(xdr::AccountFlags::RevocableFlag);
        };
        if self.set_immutable {
            set_flag(xdr::AccountFlags::ImmutableFlag);
        };
        if self.set_clawback_enabled {
            set_flag(xdr::AccountFlags::ClawbackEnabledFlag);
        };
        let mut clear_flags: u32 = 0;
        let mut clear_flag = |flag: xdr::AccountFlags| clear_flags |= flag as u32;
        if self.clear_required {
            clear_flag(xdr::AccountFlags::RequiredFlag);
        };
        if self.clear_revocable {
            clear_flag(xdr::AccountFlags::RevocableFlag);
        };
        if self.clear_immutable {
            clear_flag(xdr::AccountFlags::ImmutableFlag);
        };
        if self.clear_clawback_enabled {
            clear_flag(xdr::AccountFlags::ClawbackEnabledFlag);
        };

        let signer = if let (Some(signer), Some(signer_weight)) =
            (self.signer.clone(), self.signer_weight.as_ref())
        {
            Some(xdr::Signer {
                key: signer.into(),
                weight: u32::from(*signer_weight),
            })
        } else {
            None
        };
        xdr::OperationBody::SetOptions(xdr::SetOptionsOp {
            inflation_dest: self.inflation_dest.clone().map(Into::into),
            clear_flags: clear_flags.eq(&0).then_some(clear_flags),
            set_flags: set_flags.eq(&0).then_some(set_flags),
            master_weight: self.master_weight.map(Into::into),
            low_threshold: self.low_threshold.map(Into::into),
            med_threshold: self.med_threshold.map(Into::into),
            high_threshold: self.high_threshold.map(Into::into),
            home_domain: self.home_domain.clone().map(Into::into),
            signer,
        })
    }
}
