pub mod account_id;
pub mod asset;
pub mod asset_code;
pub mod operations;
pub mod transaction;

pub use account_id::AccountId;
pub use asset::Asset;
pub use asset_code::AssetCode;
pub use operations as ops;
pub use operations::Operation;
pub use transaction::TxExt;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Transaction contains too many operations")]
    TooManyOperations,
}
