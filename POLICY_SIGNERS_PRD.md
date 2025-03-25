# Policy Signers Generator - Product Requirements Document

## Overview
The Policy Signers Generator is a CLI tool that extends Soroban's functionality by automatically generating policy contracts from existing Soroban smart contracts. These policies implement Stellar's PolicyInterface to provide controlled access to contract functions through customizable rules and limits.

## Goals
- Simplify the creation of policy contracts for Soroban smart contracts
- Provide a standardized way to implement common access control patterns
- Reduce development time and potential security issues in policy implementation
- Enable easy customization of policy parameters

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
    
    /// Policy type (time-based, amount-based, multi-sig)
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

### Policy Parameters

#### Time-Based Policy Parameters
```json
{
  "interval": 86400,  // 24 hours in seconds
  "max_calls_per_interval": 5,
  "methods": {
    "transfer": {
      "enabled": true,
      "custom_interval": 3600  // Optional override
    },
    "swap": {
      "enabled": false
    }
  }
}
```

#### Amount-Based Policy Parameters
```json
{
  "max_amount": 1000000000,  // In stroops
  "daily_limit": 5000000000,
  "methods": {
    "transfer": {
      "enabled": true,
      "min_amount": 1000000
    }
  }
}
```

#### Multi-Signature Policy Parameters
```json
{
  "required_signatures": 2,
  "signers": [
    {
      "address": "GBXG...",
      "weight": 1
    },
    {
      "address": "GDHT...",
      "weight": 2
    }
  ],
  "expiration_period": 86400
}
```

## Implementation Details

### 1. Contract Analysis
- Parse WASM file using Soroban tools
- Extract contract interface (methods, types, specs)
- Identify state variables and their types
- Generate method metadata for policy application

### 2. Template Selection
- Choose appropriate template based on policy_type
- Load template from predefined set
- Validate template compatibility with contract

### 3. Policy Generation
- Generate Cargo.toml with dependencies
- Create policy contract structure
- Implement PolicyInterface trait
- Generate storage handling code
- Create method-specific policy checks

### 4. Testing
- Generate test cases based on policy type
- Create test vectors for common scenarios
- Include security test cases

## Milestones

### Milestone 1: Core Infrastructure 
1. **Basic CLI Structure**
   - Set up project structure
   - Implement clap command integration
   - Add interactive mode foundation
   - Basic error handling

2. **WASM Analysis Tools**
   - Implement WASM file parsing
   - Extract contract interface
   - Method detection and analysis
   - Type system integration

3. **Template System**
   - Create base template structure
   - Implement template loading system
   - Add template validation
   - Create basic policy templates

### Milestone 2: Policy Generation
1. **Time-Based Policy**
   - Implement interval tracking
   - Add cooldown mechanism
   - Create storage handling
   - Generate policy code

2. **Interactive CLI**
   - Add colorful prompts
   - Implement input validation
   - Add progress indicators
   - Create help messages

3. **Testing Framework**
   - Set up test infrastructure
   - Create test vectors
   - Add integration tests
   - Implement CI pipeline

### Milestone 3: Advanced Policies 
1. **Amount-Based Policy**
   - Implement amount tracking
   - Add limit validation
   - Create cumulative checks
   - Generate policy code

2. **Multi-Signature Policy**
   - Add signer management
   - Implement weight system
   - Create signature validation
   - Generate policy code

3. **Policy Composition**
   - Create composition system
   - Add rule combining
   - Implement priority handling
   - Add conflict resolution

### Milestone 4: Polish & Documentation 
1. **Error Handling & Validation**
   - Improve error messages
   - Add input validation
   - Implement recovery flows
   - Create validation helpers

2. **Documentation**
   - Write user guide
   - Create API documentation
   - Add example collection
   - Create tutorial videos

3. **Performance & Security**
   - Optimize template rendering
   - Add security checks
   - Implement caching
   - Create security guide

### Milestone 5: Integration & Launch
1. **Soroban Integration**
   - Add Soroban CLI hooks
   - Implement contract deployment
   - Create upgrade handling
   - Add migration tools

2. **Testing & QA**
   - Comprehensive testing
   - Security audit
   - Performance testing
   - User acceptance testing

3. **Launch Preparation**
   - Create release notes
   - Prepare launch materials
   - Set up support channels
   - Plan future roadmap

## Usage Examples

The Policy Signers Generator provides an interactive CLI experience that guides users through the policy creation process:

### Interactive Mode (Default)

```bash
$ soroban contract policy

🔹 What is the path to your contract WASM file?
> token_contract.wasm

📋 Contract Analysis Complete
Found methods: transfer, mint, burn, approve

🔹 What type of policy would you like to create?
  ⭐️ Time-based (Intervals & Cooldowns)
  Amount-based (Transaction Limits)
  Multi-signature (Multiple Approvers)
> Time-based

🔹 Configure Time-based Policy:
Base interval in seconds (e.g. 86400 for daily):
> 86400

🔹 Maximum calls per interval:
> 5

🔹 Configure method-specific rules:
Method: transfer
Enable policy for this method? (Y/n)
> Y
Use custom interval? (y/N)
> y
Custom interval in seconds:
> 3600

Method: mint
Enable policy for this method? (Y/n)
> n

Method: burn
Enable policy for this method? (Y/n)
> n

🔹 Where should we generate the policy contract?
> ./token_policy

📝 Generating policy contract...
✅ Policy contract generated successfully!

Next steps:
1. Review the generated contract in ./token_policy
2. Build using 'soroban contract build'
3. Deploy using 'soroban contract deploy'
```

### Non-Interactive Mode

For automation and scripts, you can still use the command-line arguments:

```bash
soroban contract policy \
  --wasm token_contract.wasm \
  --interactive false \
  --policy-type time-based \
  --out ./token_policy \
  --params '{
    "interval": 86400,
    "max_calls_per_interval": 5,
    "methods": {
      "transfer": {
        "enabled": true,
        "custom_interval": 3600
      }
    }
  }'
```

## Generated Project Structure

```