use stellar_xdr::curr as xdr;

pub struct BumpSequence(xdr::BumpSequenceOp);

impl BumpSequence {
    pub fn new(bump_to: impl Into<xdr::SequenceNumber>) -> Self {
        Self(xdr::BumpSequenceOp {
            bump_to: bump_to.into(),
        })
    }
}

impl super::Operation for BumpSequence {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::BumpSequence(self.0)
    }
}
