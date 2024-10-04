pub mod asset;
pub mod transaction;

pub use asset::Asset;
pub use transaction::TxExt;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Transaction contains too many operations")]
    TooManyOperations,
}
