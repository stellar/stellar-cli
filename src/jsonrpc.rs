use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, PartialEq, Clone, Hash, Eq, Deserialize, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum Id {
    /// Null
    Null,
    /// Numeric id
    Number(u64),
    /// String id
    Str(String),
}

/// JSON-RPC request object as defined in the [spec](https://www.jsonrpc.org/specification#request-object).
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Request<T> {
    /// JSON-RPC version.
    pub jsonrpc: String,
    /// Request ID
    pub id: Option<Id>,
    /// Name of the method to be invoked.
    pub method: String,
    /// Parameter values of the request.
    pub params: Option<T>,
}

/// JSON-RPC Response object as defined in the [spec](https://www.jsonrpc.org/specification#request-object).
/// TODO: Figure out a cleaner way to do this.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum Response<T, E> {
    Ok(ResultResponse<T>),
    Err(ErrorResponse<E>),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ResultResponse<T> {
    pub jsonrpc: String,
    pub id: Id,
    pub result: T,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ErrorResponse<T> {
    pub jsonrpc: String,
    pub id: Id,
    pub error: ErrorResponseError<T>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ErrorResponseError<T> {
    pub code: i64,
    pub message: String,
    pub data: Option<T>,
}
