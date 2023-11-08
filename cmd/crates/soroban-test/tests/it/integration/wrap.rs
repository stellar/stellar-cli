use soroban_cli::CommandParser;
use soroban_cli::{
    commands::{
        config::{self},
        lab::token::wrap,
    },
    utils::contract_id_hash_from_asset,
};
use soroban_test::TestEnv;

use super::util::network_passphrase;

#[tokio::test]
#[ignore]
async fn burn() {
    let sandbox = &TestEnv::default();
    let address = config::identity::address::Cmd::parse("--hd-path=0")
        .unwrap()
        .public_key()
        .unwrap();
    let asset = format!("native:{address}");
    wrap_cmd(&asset).run().await.unwrap();
    let asset = soroban_cli::utils::parsing::parse_asset(&asset).unwrap();
    let hash = contract_id_hash_from_asset(&asset, &network_passphrase().unwrap()).unwrap();
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
            .invoke(&["--id", &id, "--", "balance", "--id", &address.to_string()])
            .await
            .unwrap(),
    );

    println!(
        "{}",
        sandbox
            .invoke(&[
                "--id",
                &id,
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
            .invoke(&["--id", &id, "--", "balance", "--id", &address.to_string()])
            .await
            .unwrap(),
    );
}

fn wrap_cmd(asset: &str) -> wrap::Cmd {
    wrap::Cmd::parse_arg_vec(&[&format!("--asset={asset}")]).unwrap()
}
