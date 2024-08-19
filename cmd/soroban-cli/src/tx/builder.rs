pub mod operations;
pub use operations as ops;
pub use operations::Operation;

pub mod muxed_account;
pub use muxed_account::MuxedAccount;

pub mod account_id;
pub use account_id::AccountId;

pub mod transaction;
pub use transaction::Transaction;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Transaction contains too many operations")]
    TooManyOperations,
}
