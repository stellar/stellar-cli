use crate::xdr;

pub trait Operation {
    fn build_body(&self) -> xdr::OperationBody;
}
