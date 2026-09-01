# Stellar CLI Skill

## Overview
The Stellar CLI (`stellar`) is the command-line multi-tool for building, deploying, and interacting with Stellar smart contracts (Soroban) and the Stellar network. It's written in Rust and installed via `cargo install --locked stellar-cli`, Homebrew (`brew install stellar-cli`), or the install script.

Docs: https://developers.stellar.org/docs/tools/cli/stellar-cli
Cookbook: https://developers.stellar.org/docs/tools/cli/cookbook
Repo: https://github.com/stellar/stellar-cli

## Installation

### Install Script (macOS, Linux) — Recommended
```bash
curl -fsSL https://github.com/stellar/stellar-cli/raw/main/install.sh | sh
```
With dependency setup (installs Rust toolchain, wasm32 target, Linux dev libs):
```bash
curl -fsSL https://github.com/stellar/stellar-cli/raw/main/install.sh | sh -s -- --install-deps
```

### Homebrew (macOS, Linux)
```bash
brew install stellar-cli
```

### winget (Windows)
```bash
winget install --id Stellar.StellarCLI
```

### Cargo from Source
Requires Rust and a C build system (`build-essential` on Debian/Ubuntu).
```bash
cargo install --locked stellar-cli
```
Without optional native library features:
```bash
cargo install --locked stellar-cli --no-default-features
```

### Nix
```bash
nix run 'github:stellar/stellar-cli' -- --help
# or install permanently
nix profile install github:stellar/stellar-cli
```

### GitHub Actions
```yaml
uses: stellar/stellar-cli@v25.1.0
```

### Version Manager
[SVM (Stellar Version Manager)](https://www.npmjs.com/package/svm-cli) — install and switch between different stellar-cli versions.

### Shell Autocomplete
```bash
# Generate completions (bash, zsh, fish, powershell, elvish)
stellar completion --shell bash

# Enable in current session
source <(stellar completion --shell bash)
```

## Top-Level Subcommands
- `contract` — Build, deploy, invoke, extend, restore, and inspect smart contracts
- `keys` — Create and manage identities (key pairs, aliases)
- `network` — Configure connections to networks (testnet, mainnet, local)
- `container` — Start local networks in Docker containers (quickstart)
- `tx` — Build, sign, simulate, and send arbitrary transactions (up to 100 ops)
- `events` — Watch the network for contract events
- `xdr` — Decode and encode XDR
- `strkey` — Decode and encode strkey
- `snapshot` — Download a ledger snapshot from an archive
- `cache` — Manage transaction and contract spec cache
- `ledger` — Fetch ledger information
- `message` — Sign and verify arbitrary messages (SEP-53)
- `fees` — Fetch network fee stats and configure CLI fee settings
- `completion` — Generate shell completions (bash, zsh, fish, powershell, elvish)
- `plugin` — Manage CLI plugins
- `doctor` — Diagnose and troubleshoot CLI and network issues
- `config` — Manage CLI configuration
- `env` — Print environment variables

## Global Options
- `--config-dir <DIR>` — Config directory (default: `$XDG_CONFIG_HOME/stellar` or `~/.config/stellar`)
- `-f, --filter-logs <FILTER>` — Filter log output (or use `RUST_LOG` env var)
- `-q, --quiet` — Suppress logs including INFO
- `-v, --verbose` — Log DEBUG events
- `--very-verbose` / `--vv` — Log DEBUG and TRACE events
- `--no-cache` — Don't cache simulations/transactions

## Common Patterns

### Identity / Key Management
```bash
# Generate a new key and fund on testnet
stellar keys generate --fund alice --network testnet

# Generate without funding
stellar keys generate bob

# Add an existing public key
stellar keys add --public-key GBUG7Q... charlie

# Show address for a key
stellar keys address alice

# List all keys
stellar keys ls
```

### Network Configuration
```bash
# Set default network to testnet
stellar network use testnet

# Add a custom network
stellar network add mynet --rpc-url https://rpc.example.com --network-passphrase "My Network"

# List networks
stellar network ls
```

### Contract Build
```bash
# Build all cdylib crates in workspace for wasm32 target
stellar contract build

# Build specific package with optimization
stellar contract build --package my-contract --optimize

# Add metadata on build
stellar contract build --meta key=value

# Dry run (print commands only)
stellar contract build --print-commands-only
```

### Contract Upload (Install Wasm)
```bash
# Upload compiled wasm to the network (returns wasm hash, NOT contract address)
stellar contract upload --source alice --network testnet --wasm target/wasm32-unknown-unknown/release/my_contract.wasm
```

### Contract Deploy
```bash
# Deploy from local wasm (auto-builds in a Cargo workspace if --wasm omitted)
stellar contract deploy --source alice --network testnet --wasm my_contract.wasm --alias mycontract

# Deploy from already-uploaded wasm hash
stellar contract deploy --source alice --network testnet --wasm-hash <HASH> --alias mycontract

# Deploy with constructor args (anything after -- is passed to __constructor)
stellar contract deploy --source alice --network testnet --wasm my_contract.wasm -- --arg1 value1
```

### Contract Invoke
```bash
# Invoke a contract function
stellar contract invoke --id <CONTRACT_ID> --source alice --network testnet -- function_name --arg value

# Get help for a specific contract's functions (auto-generated from schema)
stellar contract invoke --id <CONTRACT_ID> --source alice --network testnet -- --help

# View-only (simulate, don't submit)
stellar contract invoke --id mycontract --source alice --network testnet --send=no -- get_value
```
**Key detail**: Everything after `--` (the "slop") is parsed as contract-specific arguments generated from the contract's embedded schema.

### Contract Aliases
```bash
stellar contract alias add --id <CONTRACT_ID> myalias
stellar contract alias show myalias
stellar contract alias ls
stellar contract alias remove myalias
```

### Stellar Asset Contract (SAC)
```bash
# Deploy the built-in asset contract for a Stellar asset
stellar contract asset deploy --asset USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5 --source alice --network testnet --alias usdc

# Get the SAC contract ID for an asset
stellar contract asset id --asset native --network testnet
```

### TTL Extension (State Archival)
```bash
# Extend contract instance TTL (max ~30 days = 535679 ledgers at 5s close)
stellar contract extend --id mycontract --source alice --network testnet --ledgers-to-extend 535679

# Extend contract Wasm code TTL
stellar contract extend --wasm-hash <HASH> --source alice --network testnet --ledgers-to-extend 535679

# Extend specific storage key
stellar contract extend --id mycontract --key MY_KEY --source alice --network testnet --ledgers-to-extend 535679 --durability persistent

# Extend with XDR key (for complex keys like Balance(Address))
stellar contract extend --id mycontract --key-xdr <XDR> --source alice --network testnet --ledgers-to-extend 535679 --durability persistent
```

### Contract Restore (Archived Data)
```bash
stellar contract restore --id mycontract --source alice --network testnet
stellar contract restore --id mycontract --key MY_KEY --source alice --network testnet --durability persistent
```

### Contract Info
```bash
# Get contract interface (spec)
stellar contract info interface --contract-id <CONTRACT_ID> --network testnet

# Get contract metadata
stellar contract info meta --contract-id <CONTRACT_ID> --network testnet

# From local wasm
stellar contract info interface --wasm my_contract.wasm
```

### Generate Client Bindings
```bash
# TypeScript
stellar contract bindings typescript --contract-id <CONTRACT_ID> --network testnet --output-dir ./bindings

# JSON, Rust, Python, Java, Flutter, Swift, PHP also available
stellar contract bindings json --wasm my_contract.wasm
stellar contract bindings rust --wasm my_contract.wasm
```

### Transactions (tx subcommands)
```bash
# Create a new transaction, add operations, sign, and send
stellar tx new --source alice create-account --destination bob --starting-balance 100

# Add operations to a transaction
stellar tx op add <TX_XDR> payment --destination bob --asset native --amount 50

# Sign and send
stellar tx sign <TX_XDR> --source alice
stellar tx send <TX_XDR> --network testnet
```

### Events
```bash
# Stream contract events
stellar events --id <CONTRACT_ID> --network testnet --start-ledger <LEDGER>
```

### Local Network (Container)
```bash
# Start local testnet with RPC, Horizon, and friendbot
stellar container start testnet

# Start with custom options
stellar container start testnet --ports-mapping 8001:8000
```

### XDR
```bash
stellar xdr decode --type TransactionEnvelope --base64 <BASE64>
stellar xdr encode --type TransactionEnvelope < input.json
```

## Source Account / Signing Options
Most write commands require `--source-account` (alias: `-s`, `--source`). Accepted formats:
- Identity name: `--source alice`
- Public key: `--source GDKW...`
- Muxed account: `--source MDA...`
- Secret key: `--source SC36...`
- Seed phrase: `--source "kite urban..."`

Additional signing options:
- `--sign-with-key <KEY>` — Sign with a different key than source
- `--sign-with-lab` — Sign via lab.stellar.org
- `--sign-with-ledger` — Sign with a Ledger hardware wallet
- `--hd-path <N>` — HD derivation path index (default: 0)

## Fee Options
- `--inclusion-fee <STROOPS>` — Max fee for transaction inclusion (default: 100 stroops; 1 stroop = 0.0000001 XLM)
- `--resource-fee <STROOPS>` — Override simulated resource fee for smart contract transactions
- `--instruction-leeway <N>` — Extra instruction budget headroom
- `--build-only` — Build the transaction XDR without submitting

## RPC Options (used by most network commands)
- `--rpc-url <URL>` — RPC server endpoint
- `--rpc-header <HEADER>` — Custom headers (e.g., "X-API-Key: abc123"), repeatable
- `--network-passphrase <PASSPHRASE>` — Network passphrase
- `-n, --network <NETWORK>` — Named network from config

## Important Concepts
- **Wasm Upload vs Deploy**: `contract upload` installs the Wasm bytecode and returns a hash. `contract deploy` creates a contract instance from that hash (or a local wasm) and returns a contract ID.
- **State Archival**: Contract data has a TTL. Use `contract extend` to keep it alive and `contract restore` to bring back archived entries. Max extension is ~30 days (535,679 ledgers).
- **Contract Aliases**: Use `--alias` during deploy to save friendly names. Reference them anywhere a contract ID is expected.
- **Slop Pattern**: For `contract invoke`, everything after `--` is the contract's own CLI, auto-generated from its on-chain schema.
- **Networks**: `testnet` and `mainnet` are built-in. Use `stellar network add` for custom networks. Use `stellar network use` to set a default.
- **Friendbot**: Only available on testnet. Use `stellar keys generate --fund <name> --network testnet` to create and fund in one step.

## Project Initialization
```bash
# Initialize a new Soroban contract project
stellar contract init my-project
```
This scaffolds a Rust project with the correct Cargo.toml setup for building Soroban contracts.

## Typical Development Workflow
1. `stellar contract init my-project` — Scaffold project
2. Write contract code in Rust
3. `stellar contract build` — Compile to Wasm
4. `stellar keys generate --fund alice --network testnet` — Create funded identity
5. `stellar contract deploy --source alice --network testnet --alias mycontract` — Deploy
6. `stellar contract invoke --id mycontract --source alice --network testnet -- my_function --arg value` — Interact
7. `stellar contract extend --id mycontract --source alice --network testnet --ledgers-to-extend 535679` — Keep alive
8. `stellar contract bindings typescript --contract-id mycontract --network testnet --output-dir ./client` — Generate client SDK

## Safety Rules
1. **Never** log or output private keys, secret keys, or seed phrases.
2. **Always** confirm with the wallet owner before sending a payment or creating an on-ledger account (these cost XLM).
3. **Never** sign or submit transactions on behalf of anyone other than the wallet owner.
4. Prefer **testnet** for development and testing. Only use mainnet when the user explicitly requests it.
5. Amounts are in **stroops** (1 XLM = 10,000,000 stroops) for `tx new` commands. State the human-readable amount when confirming with the user.
6. Check the account balance before sending a payment to avoid failed transactions.

## Troubleshooting
- Run `stellar doctor` to diagnose common issues
- Use `-v` or `--vv` for verbose logging
- Use `--no-cache` if stale simulation results are suspected
- Check config with `stellar config` and `stellar env`
