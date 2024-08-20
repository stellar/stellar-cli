use std::str::FromStr;

use stellar_xdr::curr as xdr;

pub struct ManageData(xdr::ManageDataOp);

impl ManageData {
    /// Creates a new `ManageDataOp` builder with the given asset.
    /// if limit is set to 0, deletes the trust line
    pub fn new(data_name: &str) -> Result<Self, xdr::Error> {
        let data_name = xdr::String64(xdr::StringM::<64>::from_str(data_name)?);
        Ok(Self(xdr::ManageDataOp {
            data_name,
            data_value: None,
        }))
    }
    pub fn set_data_value(mut self, data_value: &[u8; 64]) -> Result<Self, xdr::Error> {
        self.0.data_value = Some(xdr::DataValue(xdr::BytesM::<64>::try_from(data_value)?));
        Ok(self)
    }
}

impl super::Operation for ManageData {
    fn build_body(self) -> xdr::OperationBody {
        xdr::OperationBody::ManageData(self.0)
    }
}
