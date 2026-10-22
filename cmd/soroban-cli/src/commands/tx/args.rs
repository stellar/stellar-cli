use crate::utils::XDR_DEPTH_LIMIT;
use crate::{
    commands::{global, txn_result::TxnEnvelopeResult, HEADING_TRANSACTION},
    config::{
        self,
        address::{self, UnresolvedMuxedAccount},
        data, network, secret,
    },
    rpc::{self, Client, GetTransactionResponse},
    tx::builder::{self, asset, TxExt},
    xdr::{self, Limits, WriteXdr},
};

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    #[clap(flatten)]
    pub config: config::Args,
    /// Build the transaction and only write the base64 xdr to stdout
    #[arg(long, help_heading = HEADING_TRANSACTION)]
    pub build_only: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Tx(#[from] builder::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Address(#[from] address::Error),
    #[error(transparent)]
    Asset(#[from] asset::Error),
    #[error(transparent)]
    TxXdr(#[from] super::xdr::Error),
    #[error("invalid price format: {0}")]
    InvalidPrice(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("invalid pool ID format: {0}")]
    InvalidPoolId(String),
    #[error("invalid hex for {name}: {hex}")]
    InvalidHex { name: String, hex: String },
}

impl Args {
    pub async fn tx(&self, body: impl Into<xdr::OperationBody>) -> Result<xdr::Transaction, Error> {
        let source_account = self.source_account()?;
        let seq_num = self
            .config
            .next_sequence_number(source_account.clone().account_id())
            .await?;

        // Once we have a way to add operations this will be updated to allow for a different source account
        let operation = xdr::Operation {
            source_account: None,
            body: body.into(),
        };
        Ok(xdr::Transaction::new_tx(
            source_account,
            self.config.get_inclusion_fee()?,
            seq_num,
            operation,
        ))
    }

    pub fn client(&self) -> Result<Client, Error> {
        let network = self.config.get_network()?;
        Ok(Client::new(&network.rpc_url)?)
    }

    pub async fn handle(
        &self,
        op: impl Into<xdr::OperationBody>,
        global_args: &global::Args,
    ) -> Result<TxnEnvelopeResult<GetTransactionResponse>, Error> {
        let tx = self.tx(op).await?;
        self.handle_tx(tx, global_args).await
    }

    pub async fn handle_and_print(
        &self,
        op: impl Into<xdr::OperationBody>,
        global_args: &global::Args,
    ) -> Result<(), Error> {
        let res = self.handle(op, global_args).await?;
        if let TxnEnvelopeResult::TxnEnvelope(tx) = res {
            println!("{}", tx.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?);
        }
        Ok(())
    }

    pub async fn handle_tx(
        &self,
        tx: xdr::Transaction,
        args: &global::Args,
    ) -> Result<TxnEnvelopeResult<GetTransactionResponse>, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        if self.build_only {
            return Ok(TxnEnvelopeResult::TxnEnvelope(Box::new(tx.into())));
        }

        let print = crate::print::Print::new(args.quiet);
        let signed_tx = self.config.sign(tx, args.quiet).await?;
        let txn_resp = crate::tx::send_transaction_polling_with_events(
            &client,
            &signed_tx,
            &network.network_passphrase,
            &print,
        )
        .await?;

        if !args.no_cache {
            data::write(txn_resp.clone().try_into().unwrap(), &network.rpc_uri()?)?;
        }

        Ok(TxnEnvelopeResult::Res(txn_resp))
    }

    pub fn source_account(&self) -> Result<xdr::MuxedAccount, Error> {
        Ok(self.config.source_account()?)
    }

    pub fn resolve_muxed_address(
        &self,
        address: &UnresolvedMuxedAccount,
    ) -> Result<xdr::MuxedAccount, Error> {
        Ok(address.resolve_muxed_account(&self.config.locator, self.config.hd_path())?)
    }

    pub fn resolve_account_id(
        &self,
        address: &UnresolvedMuxedAccount,
    ) -> Result<xdr::AccountId, Error> {
        Ok(address
            .resolve_muxed_account(&self.config.locator, self.config.hd_path())?
            .account_id())
    }

    pub fn add_op(
        &self,
        op_body: impl Into<xdr::OperationBody>,
        tx_env: xdr::TransactionEnvelope,
        op_source: Option<&address::UnresolvedMuxedAccount>,
    ) -> Result<xdr::TransactionEnvelope, Error> {
        let mut source_account = None;
        if let Some(account) = op_source {
            source_account =
                Some(account.resolve_muxed_account(&self.config.locator, self.config.hd_path())?);
        }
        let op = xdr::Operation {
            source_account,
            body: op_body.into(),
        };
        Ok(super::xdr::add_op(tx_env, op)?)
    }

    pub fn resolve_asset(&self, asset: &builder::Asset) -> Result<xdr::Asset, Error> {
        Ok(asset.resolve(&self.config.locator)?)
    }

    pub fn resolve_signer_key(
        &self,
        signer_account: &UnresolvedMuxedAccount,
    ) -> Result<xdr::SignerKey, Error> {
        let resolved_account = self.resolve_account_id(signer_account)?;
        let signer_key = match resolved_account.0 {
            xdr::PublicKey::PublicKeyTypeEd25519(uint256) => xdr::SignerKey::Ed25519(uint256),
        };
        Ok(signer_key)
    }
}

/// Parse a liquidity pool ID from string supporting multiple formats:
/// 1. Address format (base32): LAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
/// 2. API format with type prefix (72 hex chars): 000000006f2179b3...
/// 3. Direct hash format (64 hex chars): 6f2179b3...
pub fn parse_liquidity_pool_id(pool_id: &str) -> Result<xdr::PoolId, Error> {
    if pool_id.starts_with('L') && pool_id.len() > 50 {
        match stellar_strkey::Strkey::from_string(pool_id) {
            Ok(stellar_strkey::Strkey::LiquidityPool(pool)) => Ok(xdr::PoolId(xdr::Hash(pool.0))),
            _ => Err(Error::InvalidPoolId(pool_id.to_string())),
        }
    } else {
        let cleaned_pool_id = if pool_id.len() == 72 && pool_id.starts_with("00000000") {
            &pool_id[8..]
        } else {
            pool_id
        };

        let pool_id_bytes =
            hex::decode(cleaned_pool_id).map_err(|_| Error::InvalidPoolId(pool_id.to_string()))?;

        let pool_id_array: [u8; 32] = pool_id_bytes
            .try_into()
            .map_err(|_| Error::InvalidPoolId(pool_id.to_string()))?;

        Ok(xdr::PoolId(xdr::Hash(pool_id_array)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_liquidity_pool_id_hex_formats() {
        let hex_id = "001041ac1d61419a62e0c0152e59f13aff7f8f65c2f04c1371ebbc662b31f4ab";
        let expected_bytes = hex::decode(hex_id).unwrap();

        // 64-char direct hex
        let pool_id = parse_liquidity_pool_id(hex_id).unwrap();
        assert_eq!(pool_id.0 .0.to_vec(), expected_bytes);

        // 72-char API hex with 00000000 prefix
        let prefix_hex = format!("00000000{hex_id}");
        let pool_id_prefixed = parse_liquidity_pool_id(&prefix_hex).unwrap();
        assert_eq!(pool_id_prefixed.0 .0.to_vec(), expected_bytes);

        // Base32 strkey format
        let mut raw_arr = [0u8; 32];
        raw_arr.copy_from_slice(&expected_bytes);
        let strkey_str =
            stellar_strkey::Strkey::LiquidityPool(stellar_strkey::LiquidityPool(raw_arr))
                .to_string();
        let pool_id_strkey = parse_liquidity_pool_id(&strkey_str).unwrap();
        assert_eq!(pool_id_strkey.0 .0.to_vec(), expected_bytes);

        // Invalid cases
        assert!(parse_liquidity_pool_id("not_a_valid_pool_id").is_err());
        assert!(parse_liquidity_pool_id("001041ac").is_err());
        assert!(parse_liquidity_pool_id("L123notastrkkey").is_err());
    }
}
