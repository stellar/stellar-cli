use std::{
    io::{stdin, Read},
    path::PathBuf,
    str::FromStr,
};

use serde_json::Value;

use crate::{
    config::locator,
    xdr::{Limits, Operation, ReadXdr, Transaction, TransactionEnvelope, TransactionV1Envelope},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to decode XDR from base64")]
    Base64Decode,
    #[error("failed to decode XDR from file: {0}")]
    FileDecode(PathBuf),
    #[error("failed to decode XDR from stdin")]
    StdinDecode,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("only transaction v1 is supported")]
    OnlyTransactionV1Supported,
    #[error("too many operations, limited to 100 operations in a transaction")]
    TooManyOperations,
}

pub enum Input {
    Xdr(String),
    Json(Value),
}

impl FromStr for Input {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains('{') {
            Ok(Self::Json(
                serde_json::from_str(s).map_err(|_| Error::StdinDecode)?,
            ))
        } else {
            Ok(Self::Xdr(s.to_string()))
        }
    }
}

impl Input {
    pub fn from_stdin() -> Result<Self, Error> {
        stdin_as_string()?.parse()
    }

    pub fn into_xdr<T: ReadXdr + serde::de::DeserializeOwned>(self) -> Result<T, Error> {
        match self {
            Self::Xdr(input) => {
                T::from_xdr_base64(input, Limits::none()).map_err(|_| Error::StdinDecode)
            }
            Self::Json(input) => serde_json::from_value(input).map_err(|_| Error::StdinDecode),
        }
    }

    // pub fn update(&mut self, locator: &locator::Args) -> Result<(), Error> {
    //     match self {
    //         Self::Xdr(input) => todo!(),
    //         Self::Json(input) => {
    //             *input = locator.read_json(input)?;
    //         },
    //     }
    //     Ok(())
    // }
}

pub fn tx_envelope_from_stdin() -> Result<TransactionEnvelope, Error> {
    Input::from_stdin()?.into_xdr()
}

fn stdin_as_string() -> Result<String, Error> {
    let mut buf = String::new();
    stdin()
        .read_to_string(&mut buf)
        .map_err(|_| Error::StdinDecode)?;
    Ok(buf.trim().to_string())
}

pub fn unwrap_envelope_v1(tx_env: TransactionEnvelope) -> Result<Transaction, Error> {
    let TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) = tx_env else {
        return Err(Error::OnlyTransactionV1Supported);
    };
    Ok(tx)
}

pub fn add_op(tx_env: TransactionEnvelope, op: Operation) -> Result<TransactionEnvelope, Error> {
    let mut tx = unwrap_envelope_v1(tx_env)?;
    let mut ops = tx.operations.to_vec();
    ops.push(op);
    tx.operations = ops.try_into().map_err(|_| Error::TooManyOperations)?;
    Ok(tx.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonpath_rust::JsonPath;
    use serde_json::Value;

    #[test]
    fn test_find_ref_paths() {
        // Read in schema from root transaction_env.json
        let input: Input = include_str!("../../../../../transaction_env.json")
            .parse()
            .unwrap();
        let Input::Json(schema) = input else {
            panic!("Expected JSON input");
        };
        let target_definition = "MuxedAccount";
        let ref_pattern = format!("#/definitions/{target_definition}");
        println!("Looking for reference: {ref_pattern}");

        // Adjusted JSONPath to retrieve all $ref values
        let jsonpath = "$..['$ref']"; // Updated syntax

        // Compile the JSONPath
        let finder: JsonPath<Value> = jsonpath.parse().unwrap_or_else(|e| {
            panic!("Failed to parse JSONPath: {e:?}");
        });

        // Use `find` to get all matching $ref values
        let Value::Array(arr) = finder.find_as_path(&schema) else {
            panic!("Expected array");
        };
        println!("All $ref values: {arr:#?}");
    }
}
