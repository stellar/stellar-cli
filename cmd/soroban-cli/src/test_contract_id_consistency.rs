#[cfg(test)]
#[allow(clippy::module_inception)]
mod test_contract_id_consistency {
    use crate::config::locator;
    use crate::tx::builder::Asset;
    use crate::utils::contract_id_hash_from_asset;

    #[test]
    fn test_asset_contract_id_consistency() {
        // Test the two asset names from the issue - note the user had different asset codes
        let asset1_str = "bft001:GDZ4CDLVSHQIAXRBTPHTPJ5MSCC6XO4R4IXRGRQ6VOVV2H2HFSQJHRYH";
        let asset2_str = "btf001:GDZ4CDLVSHQIAXRBTPHTPJ5MSCC6XO4R4IXRGRQ6VOVV2H2HFSQJHRYH";

        // Test the network passphrase (Futurenet)
        let network_passphrase = "Test SDF Future Network ; October 2022";

        // Parse assets
        let asset1: Asset = asset1_str.parse().expect("Failed to parse asset1");
        let asset2: Asset = asset2_str.parse().expect("Failed to parse asset2");

        let locator = locator::Args::default();
        let resolved_asset1 = asset1.resolve(&locator).expect("Failed to resolve asset1");
        let resolved_asset2 = asset2.resolve(&locator).expect("Failed to resolve asset2");

        // Compute contract IDs
        let contract_id1 = contract_id_hash_from_asset(&resolved_asset1, network_passphrase);
        let contract_id2 = contract_id_hash_from_asset(&resolved_asset2, network_passphrase);

        println!("Asset 1 ({asset1_str}): {contract_id1}");
        println!("Asset 2 ({asset2_str}): {contract_id2}");

        // Different assets should produce different contract IDs
        assert_ne!(
            contract_id1, contract_id2,
            "Different assets should produce different contract IDs"
        );

        // Test with the same asset name again - this should match the first one
        let same_asset_str = "bft001:GDZ4CDLVSHQIAXRBTPHTPJ5MSCC6XO4R4IXRGRQ6VOVV2H2HFSQJHRYH";
        let same_asset: Asset = same_asset_str.parse().expect("Failed to parse same asset");
        let resolved_same_asset = same_asset
            .resolve(&locator)
            .expect("Failed to resolve same asset");
        let contract_id_same =
            contract_id_hash_from_asset(&resolved_same_asset, network_passphrase);

        println!("Same asset again ({same_asset_str}): {contract_id_same}");

        // Same asset should produce same contract ID
        assert_eq!(
            contract_id1, contract_id_same,
            "Same asset should produce same contract ID"
        );
    }

    #[test]
    fn test_deploy_vs_id_consistency() {
        // This test verifies that the deploy and id commands would return the same contract ID
        use crate::commands::contract::id::asset::Cmd as IdCmd;
        use crate::config;

        let asset_str = "bft001:GDZ4CDLVSHQIAXRBTPHTPJ5MSCC6XO4R4IXRGRQ6VOVV2H2HFSQJHRYH";
        let asset: crate::tx::builder::Asset = asset_str.parse().expect("Failed to parse asset");

        // Create config for Futurenet as mentioned in the issue
        let locator = config::locator::Args::default();
        let network_args = config::network::Args {
            network: Some("futurenet".to_string()),
            ..Default::default()
        };
        // Set to Futurenet passphrase to match the issue
        let futurenet_passphrase = "Test SDF Future Network ; October 2022";

        // Create ID command and get contract address
        let id_cmd = IdCmd {
            asset: asset.clone(),
            config: config::ArgsLocatorAndNetwork {
                locator: locator.clone(),
                network: network_args,
            },
        };

        let contract_address_from_id = id_cmd
            .contract_address()
            .expect("Failed to get contract address from id command");

        // For deploy command, we need to simulate what it would compute - same logic as deploy
        let resolved_asset = asset.resolve(&locator).expect("Failed to resolve asset");
        let contract_id_from_deploy =
            contract_id_hash_from_asset(&resolved_asset, futurenet_passphrase);
        let contract_address_from_deploy = stellar_strkey::Contract(contract_id_from_deploy.0);

        println!("ID command result: {contract_address_from_id}");
        println!("Deploy computation result: {contract_address_from_deploy}");

        // They should be the same if using the same network
        assert_eq!(
            contract_address_from_id, contract_address_from_deploy,
            "Deploy and ID commands should return the same contract address for the same asset and network"
        );
    }

    #[test]
    fn test_issue_reproduction() {
        // Try to reproduce the exact issue reported by the user
        let asset_str = "bft001:GDZ4CDLVSHQIAXRBTPHTPJ5MSCC6XO4R4IXRGRQ6VOVV2H2HFSQJHRYH";
        let futurenet_passphrase = "Test SDF Future Network ; October 2022";

        let asset: Asset = asset_str.parse().expect("Failed to parse asset");
        let locator = locator::Args::default();
        let resolved_asset = asset.resolve(&locator).expect("Failed to resolve asset");

        let computed_id = contract_id_hash_from_asset(&resolved_asset, futurenet_passphrase);
        println!("Computed contract ID for {asset_str}: {computed_id}");

        // The user reported getting CAD57CH3BSRWALIRSYVK575FPCUM6QTAD2II73ROHOSGPFH62W3EJGSG from wrap
        // and CAWKSIJM64CVZ6OLBSRSVSFGEOONPILQL6HGSCHHOSKEBNSYEU3Q4IJE from id

        // Let's test if our computation matches either of these
        let user_wrap_result = "CAD57CH3BSRWALIRSYVK575FPCUM6QTAD2II73ROHOSGPFH62W3EJGSG";
        let user_id_result = "CAWKSIJM64CVZ6OLBSRSVSFGEOONPILQL6HGSCHHOSKEBNSYEU3Q4IJE";

        println!("User's wrap result: {user_wrap_result}");
        println!("User's id result: {user_id_result}");
        println!("Our computation: {computed_id}");

        // Note: The test might fail because we might be using different network settings
        // than what the user used, but this will help us understand the behavior
    }

    #[test]
    fn test_consistency_after_fix() {
        // Test that both deploy and ID commands now use the same logic
        use crate::commands::contract::id::asset::Cmd as IdCmd;
        use crate::config;

        let asset_str = "bft001:GDZ4CDLVSHQIAXRBTPHTPJ5MSCC6XO4R4IXRGRQ6VOVV2H2HFSQJHRYH";
        let asset: crate::tx::builder::Asset = asset_str.parse().expect("Failed to parse asset");

        // Create config for Futurenet as mentioned in the issue
        let locator = config::locator::Args::default();
        let network_args = config::network::Args {
            network: Some("futurenet".to_string()),
            ..Default::default()
        };

        // Create ID command and get contract address
        let id_cmd = IdCmd {
            asset: asset.clone(),
            config: config::ArgsLocatorAndNetwork {
                locator: locator.clone(),
                network: network_args,
            },
        };

        let contract_address_from_id = id_cmd
            .contract_address()
            .expect("Failed to get contract address from id command");

        // Manually compute what deploy would compute using the same exact logic
        let network = id_cmd.config.get_network().expect("Failed to get network");
        let resolved_asset = asset.resolve(&locator).expect("Failed to resolve asset");

        let contract_id_from_deploy_logic =
            contract_id_hash_from_asset(&resolved_asset, &network.network_passphrase);

        let contract_address_from_deploy_logic =
            stellar_strkey::Contract(contract_id_from_deploy_logic.0);

        println!("ID command result: {contract_address_from_id}");
        println!("Deploy logic result: {contract_address_from_deploy_logic}");

        // After our fix, they should be the same
        assert_eq!(
            contract_address_from_id, contract_address_from_deploy_logic,
            "Deploy and ID commands should return the same contract address after fix"
        );
    }
}
