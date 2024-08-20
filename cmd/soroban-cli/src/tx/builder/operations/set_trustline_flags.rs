use stellar_xdr::curr as xdr;

use crate::tx::builder;

pub struct SetTrustLineFlags(xdr::SetTrustLineFlagsOp);

impl SetTrustLineFlags {
    /// Creates a new `SetTrustLineFlagsOp` builder with the given asset.
    /// if limit is set to 0, deletes the trust line
    pub fn new(trustor: impl Into<builder::AccountId>, asset: impl Into<builder::Asset>) -> Self {
        Self(xdr::SetTrustLineFlagsOp {
            trustor: trustor.into().into(),
            asset: asset.into().into(),
            clear_flags: 0,
            set_flags: 0,
        })
    }

    fn set_clear_flags(mut self, trust_flag: xdr::TrustLineFlags) -> Self {
        self.0.clear_flags |= trust_flag as u32;
        self
    }

    fn set_set_flags(mut self, trust_flag: xdr::TrustLineFlags) -> Self {
        self.0.set_flags |= trust_flag as u32;
        self
    }

    #[must_use]
    pub fn set_authorized(self) -> Self {
        self.set_set_flags(xdr::TrustLineFlags::AuthorizedFlag)
    }

    #[must_use]
    pub fn set_authorized_to_maintain_liabilities(self) -> Self {
        self.set_set_flags(xdr::TrustLineFlags::AuthorizedToMaintainLiabilitiesFlag)
    }

    #[must_use]
    pub fn set_trustline_clawback_enabled(self) -> Self {
        self.set_set_flags(xdr::TrustLineFlags::TrustlineClawbackEnabledFlag)
    }

    #[must_use]
    pub fn clear_authorized(self) -> Self {
        self.set_clear_flags(xdr::TrustLineFlags::AuthorizedFlag)
    }

    #[must_use]
    pub fn clear_authorized_to_maintain_liabilities(self) -> Self {
        self.set_clear_flags(xdr::TrustLineFlags::AuthorizedToMaintainLiabilitiesFlag)
    }

    #[must_use]
    pub fn clear_trustline_clawback_enabled(self) -> Self {
        self.set_clear_flags(xdr::TrustLineFlags::TrustlineClawbackEnabledFlag)
    }
}

impl super::Operation for SetTrustLineFlags {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::SetTrustLineFlags(self.0)
    }
}
