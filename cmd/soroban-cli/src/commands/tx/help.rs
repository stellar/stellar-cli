pub const ACCOUNT_MERGE: &str = "Transfer XLM balance to another account and remove source account";
pub const BUMP_SEQUENCE: &str = "Bump sequence number to invalidate older transactions";
pub const CHANGE_TRUST: &str = "Create, update, or delete a trustline";
pub const CREATE_ACCOUNT: &str = "Create and fund a new account";
pub const CREATE_CLAIMABLE_BALANCE: &str =
    "Create a claimable balance that can be claimed by specified accounts";
pub const CREATE_PASSIVE_SELL_OFFER: &str = "Create a passive sell offer on the Stellar DEX";
pub const MANAGE_BUY_OFFER: &str = "Create, update, or delete a buy offer";
pub const MANAGE_DATA: &str = "Set, modify, or delete account data entries";
pub const MANAGE_SELL_OFFER: &str = "Create, update, or delete a sell offer";
pub const PATH_PAYMENT_STRICT_SEND: &str =
    "Send a payment with a different asset using path finding, specifying the send amount";
pub const PATH_PAYMENT_STRICT_RECEIVE: &str =
    "Send a payment with a different asset using path finding, specifying the receive amount";
pub const PAYMENT: &str = "Send asset to destination account";
pub const SET_OPTIONS: &str = "Set account options like flags, signers, and home domain";
pub const SET_TRUSTLINE_FLAGS: &str = "Configure authorization and trustline flags for an asset";
