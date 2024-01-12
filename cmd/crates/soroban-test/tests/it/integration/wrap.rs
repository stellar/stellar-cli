use soroban_cli::CommandParser;
use soroban_cli::{
    commands::{contract::deploy::asset, keys},
    utils::contract_id_hash_from_asset,
};
use soroban_test::TestEnv;

use super::util::network_passphrase;

#[tokio::test]
#[ignore]
async fn burn() {
    let sandbox = &TestEnv::default();
    let network_passphrase = network_passphrase().unwrap();
    println!("NETWORK_PASSPHRASE: {network_passphrase:?}");
    let address = keys::address::Cmd::parse("test")
        .unwrap()
        .public_key()
        .unwrap();
    let asset = format!("native:{address}");
    wrap_cmd(&asset).run().await.unwrap();
    let asset = soroban_cli::utils::parsing::parse_asset(&asset).unwrap();
    let hash = contract_id_hash_from_asset(&asset, &network_passphrase).unwrap();
    let id = stellar_strkey::Contract(hash.0).to_string();
    assert_eq!(
        "CAMTHSPKXZJIRTUXQP5QWJIFH3XIDMKLFAWVQOFOXPTKAW5GKV37ZC4N",
        id
    );
    assert_eq!(
        "true",
        sandbox
            .invoke(&[
                "--id",
                &id,
                "--source=test",
                "--",
                "authorized",
                "--id",
                &address.to_string()
            ])
            .await
            .unwrap()
    );
    assert_eq!(
        "\"9223372036854775807\"",
        sandbox
            .invoke(&[
                "--id",
                &id,
                "--source",
                "test",
                "--",
                "balance",
                "--id",
                &address.to_string()
            ])
            .await
            .unwrap(),
    );

    println!(
        "{}",
        sandbox
            .invoke(&[
                "--id",
                &id,
                "--source=test",
                "--",
                "burn",
                "--id",
                &address.to_string(),
                "--amount=100"
            ])
            .await
            .unwrap()
    );

    assert_eq!(
        "\"9223372036854775707\"",
        sandbox
            .invoke(&[
                "--id",
                &id,
                "--source=test",
                "--",
                "balance",
                "--id",
                &address.to_string()
            ])
            .await
            .unwrap(),
    );
}

fn wrap_cmd(asset: &str) -> asset::Cmd {
    asset::Cmd::parse_arg_vec(&["--source=test", &format!("--asset={asset}")]).unwrap()
}
