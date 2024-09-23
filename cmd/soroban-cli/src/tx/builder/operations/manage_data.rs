use stellar_xdr::curr as xdr;

use crate::tx::builder;

pub struct ManageData(xdr::ManageDataOp);

impl ManageData {
    /// Creates a new `ManageDataOp` builder with the given asset.
    /// if limit is set to 0, deletes the trust line
    pub fn new(data_name: builder::String64) -> Self {
        Self(xdr::ManageDataOp {
            data_name: data_name.into(),
            data_value: None,
        })
    }

    #[must_use]
    pub fn set_data_value(mut self, data_value: builder::Bytes64) -> Self {
        self.0.data_value = Some(xdr::DataValue(data_value.into()));
        self
    }
}

impl super::Operation for ManageData {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::ManageData(self.0)
    }
}
