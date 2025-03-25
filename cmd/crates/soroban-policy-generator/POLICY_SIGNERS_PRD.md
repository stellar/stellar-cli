## Overview
The Policy Signers Generator is a CLI tool that extends Soroban's functionality by automatically generating policy contracts from existing Soroban smart contracts. These policies implement Stellar's PolicyInterface to provide controlled access to contract functions through customizable rules and limits.

## Goals
- Simplify the creation of policy contracts for Soroban smart contracts
- Provide a standardized way to implement common access control patterns
- Reduce development time and potential security issues in policy implementation
- Enable easy customization of policy parameters
- Support Smart Wallet Interface integration for enhanced security and access control

## Non-Goals
- Creating a full policy language specification
- Supporting every possible policy combination
- Generating optimized policy code for specific use cases
- Providing a UI for policy creation

## Technical Specifications

### Command Structure

```rust
#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    // ... existing commands ...
    Policy(policy::Cmd),
}

#[derive(Debug, clap::Parser)]
pub struct PolicyCmd {
    /// Path to the contract WASM file
    #[clap(long)]
    wasm: Option<String>,
    
    /// Run in interactive mode (default: true)
    #[clap(long)]
    interactive: Option<bool>,
    
    /// Policy type (time-based, amount-based, multi-sig, smart-wallet)
    #[clap(long)]
    policy_type: Option<String>,
    
    /// Output directory for generated policy contract
    #[clap(long)]
    out: Option<String>,
    
    /// Custom policy parameters in JSON format
    #[clap(long)]
    params: Option<String>,
}
```

### Supported Policy Types

1. **Time-Based Policy**
   - Interval-based restrictions on function calls
   - Cooldown periods between calls
   - Time window restrictions

2. **Amount-Based Policy**
   - Transaction amount limits
   - Cumulative amount tracking
   - Rate limiting based on amounts

3. **Multi-Signature Policy**
   - Required number of signatures
   - Signature weights
   - Signature expiration

4. **Smart Wallet Policy**
   - Integration with Smart Wallet Interface
   - Context-based authorization
   - Function-specific rules
   - Signer key validation

### Policy Parameters

// ... existing parameters ...

#### Smart Wallet Policy Parameters
```json
{
  "smart_wallet": {
    "function_rules": {
      "transfer": {
        "enabled": true,
        "amount_limit": 10000000,
        "require_signer": true
      }
    },
    "context_validation": {
      "validate_contract_context": true,
      "allowed_contracts": ["contract_id_1", "contract_id_2"]
    },
    "signer_rules": {
      "require_specific_signers": false,
      "allowed_signers": []
    }
  }
}
```

## Smart Wallet Interface Integration

### 1. Policy Interface Implementation
- Implement the `PolicyInterface` trait from smart-wallet-interface
- Support context validation through the `policy__` function
- Handle contract-specific authorization logic
- Validate signer keys and permissions

### 2. Context Validation
- Parse and validate `Context` objects from the smart wallet
- Support `ContractContext` validation
- Implement function-specific rules
- Handle argument validation and limits

### 3. Signer Validation
- Integrate with Smart Wallet's signer system
- Support signer key validation
- Implement signer-specific rules
- Handle signer updates and removals

### 4. Security Considerations
- Proper error handling for unauthorized access
- Secure storage of signer information
- Protection against replay attacks
- Validation of contract contexts

// ... rest of the existing content ...

## Usage Examples

// ... existing examples ...

### Smart Wallet Policy Example

```bash
$ soroban contract policy

🔹 What is the path to your contract WASM file?
> token_contract.wasm

📋 Contract Analysis Complete
Found methods: transfer, mint, burn, approve

🔹 What type of policy would you like to create?
  Time-based (Intervals & Cooldowns)
  Amount-based (Transaction Limits)
  Multi-signature (Multiple Approvers)
  ⭐️ Smart Wallet (Context-based Authorization)
> Smart Wallet

🔹 Configure Smart Wallet Policy:

Function: transfer
Enable policy for this function? (Y/n)
> Y
Set amount limit (in stroops):
> 10000000
Require specific signer? (y/N)
> n

Function: mint
Enable policy for this function? (Y/n)
> Y
Set amount limit (in stroops):
> 5000000
Require specific signer? (Y/n)
> Y
Enter allowed signer keys (comma-separated):
> GBXG..., GDHT...

🔹 Configure Context Validation:
Validate contract contexts? (Y/n)
> Y
Enter allowed contract IDs (comma-separated):
> contract_id_1, contract_id_2

🔹 Where should we generate the policy contract?
> ./token_policy

📝 Generating policy contract...
✅ Policy contract generated successfully!

Next steps:
1. Review the generated contract in ./token_policy
2. Build using 'soroban contract build'
3. Deploy using 'soroban contract deploy'
```

### Non-Interactive Smart Wallet Mode

```bash
soroban contract policy \
  --wasm token_contract.wasm \
  --interactive false \
  --policy-type smart-wallet \
  --out ./token_policy \
  --params '{
    "smart_wallet": {
      "function_rules": {
        "transfer": {
          "enabled": true,
          "amount_limit": 10000000,
          "require_signer": false
        },
        "mint": {
          "enabled": true,
          "amount_limit": 5000000,
          "require_signer": true,
          "allowed_signers": ["GBXG...", "GDHT..."]
        }
      },
      "context_validation": {
        "validate_contract_context": true,
        "allowed_contracts": ["contract_id_1", "contract_id_2"]
      }
    }
  }'
```

## Smart Wallet Policy Examples

The following examples demonstrate real-world use cases for Smart Wallet policies.

### 1. Basic Token Transfer Policy

This example creates a policy that limits token transfers to 10,000 XLM per transaction:

```bash
# Interactive mode
$ soroban contract policy --wasm token_contract.wasm

🔹 Select policy type: Smart Wallet
🔹 Configure for function 'transfer':
  - Set amount limit: 100000000000 (10,000 XLM)
  - No specific signer required
  - Allow all contract contexts

# Non-interactive equivalent
$ soroban contract policy \
  --wasm token_contract.wasm \
  --interactive false \
  --policy-type smart-wallet \
  --params '{
    "smart_wallet": {
      "function_rules": {
        "transfer": {
          "enabled": true,
          "amount_limit": 100000000000,
          "require_signer": false
        }
      },
      "context_validation": {
        "validate_contract_context": false
      }
    }
  }'
```

### 2. DEX Trading Policy

This example creates a policy for a DEX contract that requires specific signers for large trades:

```bash
# Interactive mode
$ soroban contract policy --wasm dex_contract.wasm

🔹 Select policy type: Smart Wallet
🔹 Configure for function 'swap':
  - Set amount limit: 1000000000000 (100,000 XLM)
  - Require specific signers for amounts > 50,000 XLM
  - Only allow DEX contract context

# Non-interactive equivalent
$ soroban contract policy \
  --wasm dex_contract.wasm \
  --interactive false \
  --policy-type smart-wallet \
  --params '{
    "smart_wallet": {
      "function_rules": {
        "swap": {
          "enabled": true,
          "amount_limit": 1000000000000,
          "require_signer": true,
          "amount_threshold": 500000000000,
          "allowed_signers": ["GDHT...", "GBXG..."]
        }
      },
      "context_validation": {
        "validate_contract_context": true,
        "allowed_contracts": ["dex_contract_id"]
      }
    }
  }'
```

### 3. Multi-Function Treasury Policy

This example creates a policy for a treasury contract with different rules for different functions:

```bash
# Interactive mode
$ soroban contract policy --wasm treasury_contract.wasm

🔹 Select policy type: Smart Wallet
🔹 Configure multiple functions:
  - 'withdraw': Requires 2 specific signers, max 10,000 XLM
  - 'invest': Requires 1 signer, max 50,000 XLM
  - 'rebalance': Requires admin signer, no amount limit

# Non-interactive equivalent
$ soroban contract policy \
  --wasm treasury_contract.wasm \
  --interactive false \
  --policy-type smart-wallet \
  --params '{
    "smart_wallet": {
      "function_rules": {
        "withdraw": {
          "enabled": true,
          "amount_limit": 100000000000,
          "require_signer": true,
          "min_signers": 2,
          "allowed_signers": ["GDHT...", "GBXG...", "GABC..."]
        },
        "invest": {
          "enabled": true,
          "amount_limit": 500000000000,
          "require_signer": true,
          "min_signers": 1,
          "allowed_signers": ["GDHT...", "GBXG..."]
        },
        "rebalance": {
          "enabled": true,
          "require_signer": true,
          "allowed_signers": ["ADMIN_KEY"]
        }
      },
      "context_validation": {
        "validate_contract_context": true,
        "allowed_contracts": ["treasury_id", "investment_pool_id"]
      }
    }
  }'
```

### 4. Cross-Contract Policy

This example creates a policy that allows interaction between specific contracts:

```bash
# Interactive mode
$ soroban contract policy --wasm bridge_contract.wasm

🔹 Select policy type: Smart Wallet
🔹 Configure for cross-contract calls:
  - Allow calls only from verified bridges
  - Require specific signer for large transfers
  - Set different limits per chain

# Non-interactive equivalent
$ soroban contract policy \
  --wasm bridge_contract.wasm \
  --interactive false \
  --policy-type smart-wallet \
  --params '{
    "smart_wallet": {
      "function_rules": {
        "bridge_transfer": {
          "enabled": true,
          "amount_limit": 1000000000000,
          "require_signer": true,
          "chain_limits": {
            "ethereum": 1000000000000,
            "solana": 500000000000
          }
        }
      },
      "context_validation": {
        "validate_contract_context": true,
        "allowed_contracts": [
          "eth_bridge_id",
          "sol_bridge_id"
        ],
        "contract_rules": {
          "eth_bridge_id": {
            "max_daily_volume": 5000000000000
          },
          "sol_bridge_id": {
            "max_daily_volume": 2000000000000
          }
        }
      },
      "signer_rules": {
        "require_specific_signers": true,
        "allowed_signers": ["BRIDGE_ORACLE_1", "BRIDGE_ORACLE_2"]
      }
    }
  }'
```

### 5. Upgrading Existing Policy

Example of upgrading an existing policy with new rules:

```bash
# Interactive mode
$ soroban contract policy --wasm existing_contract.wasm

🔹 Existing policy detected! Choose action:
  1. Create new policy
  2. Update existing policy
> 2

🔹 Select parameters to update:
  - Update amount limits
  - Add new allowed signers
  - Modify contract contexts

# Non-interactive equivalent
$ soroban contract policy \
  --wasm existing_contract.wasm \
  --interactive false \
  --policy-type smart-wallet \
  --update true \
  --params '{
    "smart_wallet": {
      "function_rules": {
        "transfer": {
          "enabled": true,
          "amount_limit": 200000000000,
          "require_signer": true,
          "allowed_signers": ["EXISTING_SIGNER_1", "NEW_SIGNER_2"]
        }
      },
      "context_validation": {
        "validate_contract_context": true,
        "allowed_contracts": ["existing_contract_id", "new_contract_id"]
      }
    }
  }'
```

These examples showcase different use cases for Smart Wallet policies:

1. **Basic Token Transfers**: Simple amount limits without specific signer requirements
2. **DEX Trading**: Complex rules based on transaction amounts with specific signer requirements
3. **Treasury Management**: Multi-function policies with different authorization levels
4. **Cross-Contract Integration**: Policies for managing contract interactions and chain-specific limits
5. **Policy Updates**: Workflow for modifying existing policies with new rules and requirements

Each example demonstrates both interactive and non-interactive usage, making the tool suitable for both development and automation workflows.

// ... rest of existing content ... 