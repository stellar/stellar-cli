use hex;
use std::ffi::OsString;

use crate::{commands::global, config::network, utils::transaction_env_hash};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TxEnvelopeFromStdin(#[from] super::xdr::Error),
    #[error(transparent)]
    XdrToBase64(#[from] crate::xdr::Error),
    #[error(transparent)]
    Config(#[from] network::Error),
}

// Command to return the transaction hash submitted to a network
/// e.g. `stellar tx hash file.txt` or `cat file.txt | stellar tx hash`
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    /// Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty
    #[arg()]
    pub tx_xdr: Option<OsString>,

    #[clap(flatten)]
    pub network: network::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let tx_env = super::xdr::tx_envelope_from_input(&self.tx_xdr)?;
        let network = &self.network.resolve(&global_args.locator, false)?;
        println!(
            "{}",
            hex::encode(transaction_env_hash(&tx_env, &network.network_passphrase)?)
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        FeeBumpTransaction, FeeBumpTransactionEnvelope, FeeBumpTransactionExt,
        FeeBumpTransactionInnerTx, MuxedAccount, SequenceNumber, Transaction, TransactionEnvelope,
        TransactionExt, TransactionV1Envelope, Uint256,
    };

    #[test]
    fn test_tx_hash_fee_bump() {
        let v1_env = TransactionV1Envelope {
            tx: Transaction {
                source_account: MuxedAccount::Ed25519(Uint256([0u8; 32])),
                fee: 100,
                seq_num: SequenceNumber(1),
                cond: crate::xdr::Preconditions::None,
                memo: crate::xdr::Memo::None,
                operations: [].try_into().unwrap(),
                ext: TransactionExt::V0,
            },
            signatures: [].try_into().unwrap(),
        };

        let fee_bump_env = FeeBumpTransactionEnvelope {
            tx: FeeBumpTransaction {
                fee_source: MuxedAccount::Ed25519(Uint256([1u8; 32])),
                fee: 10000,
                inner_tx: FeeBumpTransactionInnerTx::Tx(v1_env),
                ext: FeeBumpTransactionExt::V0,
            },
            signatures: [].try_into().unwrap(),
        };

        let tx_env = TransactionEnvelope::TxFeeBump(fee_bump_env);
        let network_passphrase = "Test SDF Network ; September 2015";
        let hash_result = transaction_env_hash(&tx_env, network_passphrase);
        assert!(hash_result.is_ok());
        let hash = hash_result.unwrap();
        assert_eq!(hash.len(), 32);
        let hex_hash = hex::encode(hash);
        assert_eq!(hex_hash.len(), 64);
        assert!(hex_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
