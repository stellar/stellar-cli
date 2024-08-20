pub mod account_id;
pub mod asset;
pub mod asset_code;
pub mod bytesm;
pub mod muxed_account;
pub mod operations;
pub mod stringm;
pub mod transaction;

pub use account_id::AccountId;
pub use asset::Asset;
pub use asset_code::AssetCode;
pub use bytesm::*;
pub use muxed_account::MuxedAccount;
pub use operations as ops;
pub use operations::Operation;
pub use stringm::*;
pub use transaction::Transaction;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Transaction contains too many operations")]
    TooManyOperations,
}
