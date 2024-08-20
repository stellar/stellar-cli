use stellar_xdr::curr as xdr;

pub struct ChangeTrust(xdr::ChangeTrustOp);

impl ChangeTrust {
    /// Creates a new `ChangeTrustOp` builder with the given asset.
    /// if limit is set to 0, deletes the trust line
    pub fn new(line: xdr::ChangeTrustAsset, limit: i64) -> Self {
        Self(xdr::ChangeTrustOp { line, limit })
    }
}

impl super::Operation for ChangeTrust {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::ChangeTrust(self.0)
    }
}
