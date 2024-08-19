use std::str::FromStr;

use crate::utils::parsing as asset;
use crate::xdr;

#[derive(Clone, Debug)]
pub struct Asset(pub xdr::Asset);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    AssetParsing(#[from] asset::Error),
}

impl FromStr for Asset {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Asset(asset::parse_asset(value)?))
    }
}

impl From<Asset> for xdr::Asset {
    fn from(builder: Asset) -> Self {
        builder.0
    }
}
