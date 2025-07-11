#[cfg(test)]
mod test_contract_id_consistency {
    use crate::utils::contract_id_hash_from_asset;
    use crate::tx::builder::Asset;
    use crate::config::locator;

    #[test]
    fn test_asset_contract_id_consistency() {
        // Test the two asset names from the issue
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
        
        println!("Asset 1 ({}): {}", asset1_str, contract_id1);
        println!("Asset 2 ({}): {}", asset2_str, contract_id2);
        
        // Different assets should produce different contract IDs
        assert_ne!(contract_id1, contract_id2, "Different assets should produce different contract IDs");
        
        // Test with the same asset name again
        let same_asset_str = "bft001:GDZ4CDLVSHQIAXRBTPHTPJ5MSCC6XO4R4IXRGRQ6VOVV2H2HFSQJHRYH";
        let same_asset: Asset = same_asset_str.parse().expect("Failed to parse same asset");
        let resolved_same_asset = same_asset.resolve(&locator).expect("Failed to resolve same asset");
        let contract_id_same = contract_id_hash_from_asset(&resolved_same_asset, network_passphrase);
        
        println!("Same asset again ({}): {}", same_asset_str, contract_id_same);
        
        // Same asset should produce same contract ID
        assert_eq!(contract_id1, contract_id_same, "Same asset should produce same contract ID");
    }

    #[test] 
    fn test_deploy_vs_id_consistency() {
        // This test verifies that the deploy and id commands would return the same contract ID
        use crate::commands::contract::deploy::asset::Cmd as DeployCmd;
        use crate::commands::contract::id::asset::Cmd as IdCmd;
        use crate::config;
        
        let asset_str = "bft001:GDZ4CDLVSHQIAXRBTPHTPJ5MSCC6XO4R4IXRGRQ6VOVV2H2HFSQJHRYH";
        let asset: crate::tx::builder::Asset = asset_str.parse().expect("Failed to parse asset");
        
        // Create mock config - we'll just use the locator part for asset resolution
        let locator = config::locator::Args::default();
        
        // Create ID command and get contract address
        let id_cmd = IdCmd {
            asset: asset.clone(),
            config: config::ArgsLocatorAndNetwork {
                locator: locator.clone(),
                network: config::network::Args::default(),
            },
        };
        
        let contract_address_from_id = id_cmd.contract_address().expect("Failed to get contract address from id command");
        
        // For deploy command, we need to simulate what it would compute
        let resolved_asset = asset.resolve(&locator).expect("Failed to resolve asset");
        let network_passphrase = "Test SDF Network ; September 2015"; // default network
        let contract_id_from_deploy = contract_id_hash_from_asset(&resolved_asset, network_passphrase);
        let contract_address_from_deploy = stellar_strkey::Contract(contract_id_from_deploy.0);
        
        println!("ID command result: {}", contract_address_from_id);
        println!("Deploy computation result: {}", contract_address_from_deploy);
        
        // They should be the same
        assert_eq!(contract_address_from_id, contract_address_from_deploy, 
                   "Deploy and ID commands should return the same contract address");
    }
}