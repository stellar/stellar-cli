use stellar_xdr::curr as xdr;

pub struct EndSponsoringFutureReserves;

impl EndSponsoringFutureReserves {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for EndSponsoringFutureReserves {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Operation for EndSponsoringFutureReserves {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::EndSponsoringFutureReserves
    }
}
