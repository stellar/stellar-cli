use stellar_xdr::curr as xdr;

use crate::tx::builder;

pub struct AllowTrust(xdr::AllowTrustOp);

impl AllowTrust {
    pub fn new(trustor: impl Into<builder::AccountId>, asset: xdr::AssetCode) -> Self {
        Self(xdr::AllowTrustOp {
            trustor: trustor.into().into(),
            asset,
            authorize: 0,
        })
    }

    fn set_authorize(mut self, trust_flag: xdr::TrustLineFlags) -> Self {
        self.0.authorize |= trust_flag as u32;
        self
    }

    #[must_use]
    pub fn set_authorized(self) -> Self {
        self.set_authorize(xdr::TrustLineFlags::AuthorizedFlag)
    }

    #[must_use]
    pub fn set_authorized_to_maintain_liabilities(self) -> Self {
        self.set_authorize(xdr::TrustLineFlags::AuthorizedToMaintainLiabilitiesFlag)
    }

    #[must_use]
    pub fn set_trustline_clawback_enabled(self) -> Self {
        self.set_authorize(xdr::TrustLineFlags::TrustlineClawbackEnabledFlag)
    }
}

impl super::Operation for AllowTrust {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::AllowTrust(self.0)
    }
}
