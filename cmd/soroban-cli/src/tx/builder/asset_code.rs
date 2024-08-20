use std::str::FromStr;

use crate::utils::parsing as asset;
use crate::xdr;

#[derive(Clone, Debug)]
pub struct AssetCode(pub xdr::AssetCode);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    AssetCodeParsing(#[from] asset::Error),
}

impl FromStr for AssetCode {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(AssetCode(asset::parse_asset_code(value)?))
    }
}

impl From<AssetCode> for xdr::AssetCode {
    fn from(builder: AssetCode) -> Self {
        builder.0
    }
}
