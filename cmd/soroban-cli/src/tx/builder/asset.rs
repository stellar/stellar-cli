use std::str::FromStr;

use crate::xdr::{self, AlphaNum12, AlphaNum4, AssetCode};

#[derive(Clone, Debug)]
pub struct Asset(pub xdr::Asset);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot parse asset: {0}, expected format: 'native' or 'code:issuer'")]
    CannotParseAsset(String),

    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

impl FromStr for Asset {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "native" {
            return Ok(Asset(xdr::Asset::Native));
        }
        let mut iter = value.splitn(2, ':');
        let (Some(code), Some(issuer), None) = (iter.next(), iter.next(), iter.next()) else {
            return Err(Error::CannotParseAsset(value.to_string()));
        };
        let issuer = issuer.parse()?;
        Ok(Asset(match code.parse()? {
            AssetCode::CreditAlphanum4(asset_code) => {
                xdr::Asset::CreditAlphanum4(AlphaNum4 { asset_code, issuer })
            }
            AssetCode::CreditAlphanum12(asset_code) => {
                xdr::Asset::CreditAlphanum12(AlphaNum12 { asset_code, issuer })
            }
        }))
    }
}

impl From<Asset> for xdr::Asset {
    fn from(builder: Asset) -> Self {
        builder.0
    }
}

impl From<&Asset> for xdr::Asset {
    fn from(builder: &Asset) -> Self {
        builder.clone().into()
    }
}
