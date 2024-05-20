use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};
use soroban_cli::{
    commands::{contract::install, tx},
    fee,
};
use soroban_sdk::xdr::{
    self, AccountEntry, Limits, ReadXdr, Transaction, TransactionEnvelope, TransactionV1Envelope,
    VecM, WriteXdr,
};
use soroban_test::{AssertExt, TestEnv};

use crate::{
    integration::util::{deploy_contract, HELLO_WORLD},
    util::LOCAL_NETWORK_PASSPHRASE,
};

#[tokio::test]
async fn txn_simulate() {
    
    let sandbox = &TestEnv::new();
    let xdr_base64 = deploy_contract(sandbox, HELLO_WORLD, true).await;
    println!("{xdr_base64}");
    let cmd = tx::simulate::Cmd::default();
    let tx_env = TransactionEnvelope::from_xdr_base64(&xdr_base64, Limits::none()).unwrap();
    let TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) = &tx_env else {
        panic!("Only transaction v1 is supported")
    };
    let assembled = cmd.simulate(tx, &sandbox.client()).await.unwrap();
    let assembled_str = sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .write_stdin(xdr_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    println!("{assembled_str}");
    assert_eq!(
        assembled
            .transaction()
            .to_xdr_base64(Limits::none())
            .unwrap(),
        assembled_str
    );
}

#[tokio::test]
async fn txn_send() {
    
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .args(["--wasm", HELLO_WORLD.path().as_os_str().to_str().unwrap()])
        .assert()
        .success();

    let xdr_base64 = deploy_contract(sandbox, HELLO_WORLD, true).await;
    println!("{xdr_base64}");
    let tx_env = TransactionEnvelope::from_xdr_base64(&xdr_base64, Limits::none()).unwrap();
    let TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) = &tx_env else {
        panic!("Only transaction v1 is supported")
    };
    let assembled = tx::simulate::Cmd::default()
        .simulate(tx, &sandbox.client())
        .await
        .unwrap();
    let mut tx = assembled.transaction().clone();
    let address = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test")
        .assert()
        .stdout_as_str();
    let secret_key = sandbox
        .new_assert_cmd("keys")
        .arg("show")
        .arg("test")
        .assert()
        .stdout_as_str();
    println!("Secret key: {secret_key}");
    let key = stellar_strkey::ed25519::PrivateKey::from_string(&secret_key).unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&key.0);
    let xdr::AccountEntry { seq_num, .. } = sandbox.client().get_account(&address).await.unwrap();
    tx.seq_num = xdr::SequenceNumber(seq_num.0 + 1);
    let tx_env = sign(tx, &key, LOCAL_NETWORK_PASSPHRASE).unwrap();

    println!(
        "Transaction to send:\n{}",
        tx_env.to_xdr_base64(Limits::none()).unwrap()
    );

    let tx_env = assembled.sign(&key, LOCAL_NETWORK_PASSPHRASE).unwrap();
    let assembled_str = sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .arg("--source=test")
        .write_stdin(tx_env.to_xdr_base64(Limits::none()).unwrap())
        .assert()
        .success()
        .stdout_as_str();
    println!("Transaction sent: {assembled_str}");
}

fn sign(
    tx: Transaction,
    key: &ed25519_dalek::SigningKey,
    network_passphrase: &str,
) -> Result<TransactionEnvelope, xdr::Error> {
    let tx_hash = hash(&tx, network_passphrase).unwrap();
    let tx_signature = key.sign(&tx_hash);

    let decorated_signature = xdr::DecoratedSignature {
        hint: xdr::SignatureHint(key.verifying_key().to_bytes()[28..].try_into()?),
        signature: xdr::Signature(tx_signature.to_bytes().try_into()?),
    };

    Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: vec![decorated_signature].try_into()?,
    }))
}

pub fn hash(tx: &Transaction, network_passphrase: &str) -> Result<[u8; 32], xdr::Error> {
    let signature_payload = xdr::TransactionSignaturePayload {
        network_id: xdr::Hash(Sha256::digest(network_passphrase).into()),
        tagged_transaction: xdr::TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone()),
    };
    Ok(Sha256::digest(signature_payload.to_xdr(Limits::none())?).into())
}
