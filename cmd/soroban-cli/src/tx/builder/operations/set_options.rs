use std::str::FromStr;

use stellar_xdr::curr as xdr;

use crate::tx::builder;

pub struct SetOptions(xdr::SetOptionsOp);

impl Default for SetOptions {
    fn default() -> Self {
        Self(xdr::SetOptionsOp {
            inflation_dest: None,
            clear_flags: None,
            set_flags: None,
            master_weight: None,
            low_threshold: None,
            med_threshold: None,
            high_threshold: None,
            home_domain: None,
            signer: None,
        })
    }
}

impl SetOptions {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn set_inflation_dest(mut self, inflation_dest: impl Into<builder::AccountId>) -> Self {
        self.0.inflation_dest = Some(inflation_dest.into().into());
        self
    }

    fn set_flag(mut self, flag: xdr::AccountFlags) -> Self {
        let flags = self.0.set_flags.unwrap_or(0);
        self.0.set_flags = Some(flags | flag as u32);
        self
    }

    fn clear_flag(mut self, flag: xdr::AccountFlags) -> Self {
        let flags = self.0.clear_flags.unwrap_or(0);
        self.0.clear_flags = Some(flags | flag as u32);
        self
    }

    #[must_use]
    pub fn set_required_flag(self) -> Self {
        self.set_flag(xdr::AccountFlags::RequiredFlag)
    }

    #[must_use]
    pub fn set_revocable_flag(self) -> Self {
        self.set_flag(xdr::AccountFlags::RevocableFlag)
    }

    #[must_use]
    pub fn set_immutable_flag(self) -> Self {
        self.set_flag(xdr::AccountFlags::ImmutableFlag)
    }

    #[must_use]
    pub fn set_clawback_enabled_flag(self) -> Self {
        self.set_flag(xdr::AccountFlags::ClawbackEnabledFlag)
    }

    #[must_use]
    pub fn clear_required_flag(self) -> Self {
        self.clear_flag(xdr::AccountFlags::RequiredFlag)
    }

    #[must_use]
    pub fn clear_revocable_flag(self) -> Self {
        self.clear_flag(xdr::AccountFlags::RevocableFlag)
    }

    #[must_use]
    pub fn clear_immutable_flag(self) -> Self {
        self.clear_flag(xdr::AccountFlags::ImmutableFlag)
    }

    #[must_use]
    pub fn clear_clawback_enabled_flag(self) -> Self {
        self.clear_flag(xdr::AccountFlags::ClawbackEnabledFlag)
    }

    #[must_use]
    pub fn set_master_weight(mut self, master_weight: u8) -> Self {
        self.0.master_weight = Some(master_weight.into());
        self
    }

    #[must_use]
    pub fn set_low_threshold(mut self, low_threshold: u8) -> Self {
        self.0.low_threshold = Some(low_threshold.into());
        self
    }

    #[must_use]
    pub fn set_med_threshold(mut self, med_threshold: u8) -> Self {
        self.0.med_threshold = Some(med_threshold.into());
        self
    }

    #[must_use]
    pub fn set_high_threshold(mut self, high_threshold: u8) -> Self {
        self.0.high_threshold = Some(high_threshold.into());
        self
    }

    pub fn set_home_domain(mut self, home_domain: &fqdn::FQDN) -> Result<Self, xdr::Error> {
        self.0.home_domain = Some(xdr::String32::from(xdr::StringM::<32>::from_str(
            &home_domain.to_string(),
        )?));
        Ok(self)
    }

    #[must_use]
    pub fn set_signer(mut self, signer: xdr::Signer) -> Self {
        self.0.signer = Some(signer);
        self
    }
}

impl super::Operation for SetOptions {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::SetOptions(self.0)
    }
}
