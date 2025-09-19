use soroban_cli::xdr::{self, ReadXdr};

use soroban_test::TestEnv;

use crate::integration::util::setup_accounts;

#[tokio::test]
async fn manage_data() {
    let sandbox = &TestEnv::new();
    let (test, _) = setup_accounts(sandbox);
    let client = sandbox.network.rpc_client().unwrap();
    let key = "test";
    let value = "beefface";
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-data",
            "--data-name",
            key,
            "--data-value",
            value,
        ])
        .assert()
        .success();
    let account_id = xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(
        stellar_strkey::ed25519::PublicKey::from_string(&test)
            .unwrap()
            .0,
    )));
    let orig_data_name: xdr::StringM<64> = key.parse().unwrap();
    let res = client
        .get_ledger_entries(&[xdr::LedgerKey::Data(xdr::LedgerKeyData {
            account_id,
            data_name: orig_data_name.clone().into(),
        })])
        .await
        .unwrap();
    let value_res = res.entries.as_ref().unwrap().first().unwrap();
    let ledeger_entry_data =
        xdr::LedgerEntryData::from_xdr_base64(&value_res.xdr, xdr::Limits::none()).unwrap();
    let xdr::LedgerEntryData::Data(xdr::DataEntry {
        data_value,
        data_name,
        ..
    }) = ledeger_entry_data
    else {
        panic!("Expected DataEntry");
    };
    assert_eq!(data_name, orig_data_name.into());
    assert_eq!(hex::encode(data_value.0.to_vec()), value);
}
