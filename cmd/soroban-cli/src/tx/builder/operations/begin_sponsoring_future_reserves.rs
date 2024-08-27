use stellar_xdr::curr as xdr;

pub struct BeginSponsoringFutureReserves(xdr::BeginSponsoringFutureReservesOp);

impl BeginSponsoringFutureReserves {
    pub fn new(sponsored_id: impl Into<xdr::AccountId>) -> Self {
        Self(xdr::BeginSponsoringFutureReservesOp {
            sponsored_id: sponsored_id.into(),
        })
    }
}

impl super::Operation for BeginSponsoringFutureReserves {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::BeginSponsoringFutureReserves(self.0)
    }
}
