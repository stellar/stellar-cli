use soroban_test::{AssertExt, TestEnv};

// Transaction envelope from https://github.com/stellar/stellar-cli/issues/2455.
const TX_ENVELOPE: &str = "AAAAAgAAAAAnEWb4nxMsQhdnS16MqBxwItF3X/JNRkfxu8eyEQyfegAAAGQAAAAAAAAAAQAAAAAAAAAAAAAAAQAAAAAAAAABAAAAABk++lrW4HlZwaWwYdYvJPCil1ibyI/VTH6WKda2sOC+AAAAAAAAAAAAAABkAAAAAAAAAAA=";
const SECRET_KEY: &str = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";

// `tx sign` only mixes the network passphrase into the transaction hash, so it
// must not require an RPC URL (regression test for #2455).
#[tokio::test]
async fn tx_sign_requires_only_network_passphrase() {
    let sandbox = &TestEnv::new();

    let output = sandbox
        .new_assert_cmd("tx")
        .args([
            "sign",
            TX_ENVELOPE,
            "--network-passphrase",
            "specified manually",
            "--sign-with-key",
            SECRET_KEY,
        ])
        .assert()
        .success()
        .stdout_as_str();

    // A signed envelope is longer than the unsigned one it was built from.
    assert!(output.trim().len() > TX_ENVELOPE.len());
}

// Same expectation for `tx hash`, which also only needs the passphrase.
#[tokio::test]
async fn tx_hash_requires_only_network_passphrase() {
    let sandbox = &TestEnv::new();

    let output = sandbox
        .new_assert_cmd("tx")
        .args([
            "hash",
            TX_ENVELOPE,
            "--network-passphrase",
            "specified manually",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let hash = output.trim();
    assert_eq!(hash.len(), 64, "expected a 32-byte hex hash, got: {hash}");
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

// `tx hash` must hash a fee bump envelope with the fee bump signature payload,
// instead of rejecting it as a non-v1 envelope (regression test for #2769).
#[tokio::test]
async fn tx_hash_fee_bump() {
    use sha2::{Digest, Sha256};
    use soroban_cli::xdr::{
        FeeBumpTransaction, FeeBumpTransactionEnvelope, FeeBumpTransactionExt,
        FeeBumpTransactionInnerTx, Hash, Limits, MuxedAccount, ReadXdr, TransactionEnvelope,
        TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction, Uint256,
        WriteXdr,
    };

    let passphrase = "specified manually";
    let TransactionEnvelope::Tx(inner) =
        TransactionEnvelope::from_xdr_base64(TX_ENVELOPE, Limits::none()).unwrap()
    else {
        panic!("expected a v1 transaction envelope");
    };
    let fee_bump_tx = FeeBumpTransaction {
        fee_source: MuxedAccount::Ed25519(Uint256([7; 32])),
        fee: 10_000,
        inner_tx: FeeBumpTransactionInnerTx::Tx(inner),
        ext: FeeBumpTransactionExt::V0,
    };
    let fee_bump_env = TransactionEnvelope::TxFeeBump(FeeBumpTransactionEnvelope {
        tx: fee_bump_tx.clone(),
        signatures: [].try_into().unwrap(),
    })
    .to_xdr_base64(Limits::none())
    .unwrap();

    let payload = TransactionSignaturePayload {
        network_id: Hash(Sha256::digest(passphrase).into()),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::TxFeeBump(fee_bump_tx),
    };
    let expected = hex::encode(Sha256::digest(payload.to_xdr(Limits::none()).unwrap()));

    let sandbox = &TestEnv::new();
    let hash_of = |envelope: &str| {
        sandbox
            .new_assert_cmd("tx")
            .args(["hash", envelope, "--network-passphrase", passphrase])
            .assert()
            .success()
            .stdout_as_str()
            .trim()
            .to_string()
    };

    let fee_bump_hash = hash_of(&fee_bump_env);
    assert_eq!(fee_bump_hash, expected);
    assert_ne!(fee_bump_hash, hash_of(TX_ENVELOPE));
}
