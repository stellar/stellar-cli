use std::str::FromStr;

#[derive(Clone, Debug, Copy)]
pub struct Amount(i64);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot start or end with `_`: {0}")]
    CannotStartOrEndWithUnderscore(String),
    #[error(transparent)]
    IntParse(#[from] std::num::ParseIntError),
}

impl FromStr for Amount {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.starts_with('_') || value.ends_with('_') {
            return Err(Error::CannotStartOrEndWithUnderscore(value.to_string()));
        }
        Ok(Self(value.replace('_', "").parse::<i64>()?))
    }
}

impl From<Amount> for i64 {
    fn from(builder: Amount) -> Self {
        builder.0
    }
}

impl From<&Amount> for i64 {
    fn from(builder: &Amount) -> Self {
        (*builder).into()
    }
}
