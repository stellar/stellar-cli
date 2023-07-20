# Command-Line Help for `soroban`

This document contains the help content for the `soroban` command-line program.

**Command Overview:**

* [`soroban`↴](#soroban)
* [`soroban contract`↴](#soroban-contract)
* [`soroban contract bindings`↴](#soroban-contract-bindings)
* [`soroban contract bindings json`↴](#soroban-contract-bindings-json)
* [`soroban contract bindings rust`↴](#soroban-contract-bindings-rust)
* [`soroban contract bindings typescript`↴](#soroban-contract-bindings-typescript)
* [`soroban contract build`↴](#soroban-contract-build)
* [`soroban contract bump`↴](#soroban-contract-bump)
* [`soroban contract deploy`↴](#soroban-contract-deploy)
* [`soroban contract fetch`↴](#soroban-contract-fetch)
* [`soroban contract inspect`↴](#soroban-contract-inspect)
* [`soroban contract install`↴](#soroban-contract-install)
* [`soroban contract invoke`↴](#soroban-contract-invoke)
* [`soroban contract optimize`↴](#soroban-contract-optimize)
* [`soroban contract read`↴](#soroban-contract-read)
* [`soroban contract restore`↴](#soroban-contract-restore)
* [`soroban config`↴](#soroban-config)
* [`soroban config identity`↴](#soroban-config-identity)
* [`soroban config identity add`↴](#soroban-config-identity-add)
* [`soroban config identity address`↴](#soroban-config-identity-address)
* [`soroban config identity generate`↴](#soroban-config-identity-generate)
* [`soroban config identity ls`↴](#soroban-config-identity-ls)
* [`soroban config identity rm`↴](#soroban-config-identity-rm)
* [`soroban config identity show`↴](#soroban-config-identity-show)
* [`soroban config network`↴](#soroban-config-network)
* [`soroban config network add`↴](#soroban-config-network-add)
* [`soroban config network rm`↴](#soroban-config-network-rm)
* [`soroban config network ls`↴](#soroban-config-network-ls)
* [`soroban events`↴](#soroban-events)
* [`soroban lab`↴](#soroban-lab)
* [`soroban lab token`↴](#soroban-lab-token)
* [`soroban lab token wrap`↴](#soroban-lab-token-wrap)
* [`soroban lab token id`↴](#soroban-lab-token-id)
* [`soroban lab xdr`↴](#soroban-lab-xdr)
* [`soroban lab xdr dec`↴](#soroban-lab-xdr-dec)
* [`soroban version`↴](#soroban-version)
* [`soroban completion`↴](#soroban-completion)

## `soroban`

Build, deploy, & interact with contracts; set identities to sign with; configure networks; generate keys; and more.

Intro: https://soroban.stellar.org
CLI Reference: https://github.com/stellar/soroban-tools/tree/main/docs/soroban-cli-full-docs.md

The easiest way to get started is to generate a new identity:

    soroban config identity generate alice

You can use identities with the `--source` flag in other commands later.

Commands that relate to smart contract interactions are organized under the `contract` subcommand. List them:

    soroban contract --help

A Soroban contract has its interface schema types embedded in the binary that gets deployed on-chain, making it possible to dynamically generate a custom CLI for each. `soroban contract invoke` makes use of this:

    soroban contract invoke --id 1 --source alice -- --help

Anything after the `--` double dash (the "slop") is parsed as arguments to the contract-specific CLI, generated on-the-fly from the embedded schema. For the hello world example, with a function called `hello` that takes one string argument `to`, here's how you invoke it:

    soroban contract invoke --id 1 --source alice -- hello --to world

Full CLI reference: https://github.com/stellar/soroban-tools/tree/main/docs/soroban-cli-full-docs.md

**Usage:** `soroban [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `contract` — Tools for smart contract developers
* `config` — Read and update config
* `events` — Watch the network for contract events
* `lab` — Experiment with early features and expert tools
* `version` — Print version information
* `completion` — Print shell completion code for the specified shell

###### **Options:**

* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `-f`, `--filter-logs <FILTER_LOGS>` — Filter logs output. To turn on "soroban_cli::log::footprint=debug" or off "=off". Can also use env var `RUST_LOG`
* `-q`, `--quiet` — Do not write logs to stderr including `INFO`
* `-v`, `--verbose` — Log DEBUG events
* `--very-verbose` — Log DEBUG and TRACE events
* `--list` — List installed plugins. E.g. `soroban-hello`



## `soroban contract`

Tools for smart contract developers

**Usage:** `soroban contract <COMMAND>`

###### **Subcommands:**

* `bindings` — Generate code client bindings for a contract
* `build` — Build a contract from source
* `bump` — Extend the expiry ledger of a contract-data ledger entry
* `deploy` — Deploy a contract
* `fetch` — Fetch a contract's Wasm binary from a network or local sandbox
* `inspect` — Inspect a WASM file listing contract functions, meta, etc
* `install` — Install a WASM file to the ledger without creating a contract instance
* `invoke` — Invoke a contract function
* `optimize` — Optimize a WASM file
* `read` — Print the current value of a contract-data ledger entry
* `restore` — Restore an evicted value for a contract-data legder entry



## `soroban contract bindings`

Generate code client bindings for a contract

**Usage:** `soroban contract bindings <COMMAND>`

###### **Subcommands:**

* `json` — Generate Json Bindings
* `rust` — Generate Rust bindings
* `typescript` — Generate a TypeScript / JavaScript package



## `soroban contract bindings json`

Generate Json Bindings

**Usage:** `soroban contract bindings json --wasm <WASM>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary



## `soroban contract bindings rust`

Generate Rust bindings

**Usage:** `soroban contract bindings rust --wasm <WASM>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary



## `soroban contract bindings typescript`

Generate a TypeScript / JavaScript package

**Usage:** `soroban contract bindings typescript [OPTIONS] --wasm <WASM> --output-dir <OUTPUT_DIR> --contract-name <CONTRACT_NAME> --contract-id <CONTRACT_ID>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary
* `--output-dir <OUTPUT_DIR>` — where to place generated project
* `--contract-name <CONTRACT_NAME>`
* `--contract-id <CONTRACT_ID>`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config



## `soroban contract build`

Build a contract from source

Builds all crates that are referenced by the cargo manifest (Cargo.toml) that have cdylib as their crate-type. Crates are built for the wasm32 target. Unless configured otherwise, crates are built with their default features and with their release profile.

To view the commands that will be executed, without executing them, use the --print-commands-only option.

**Usage:** `soroban contract build [OPTIONS]`

###### **Options:**

* `--manifest-path <MANIFEST_PATH>` — Path to Cargo.toml

  Default value: `Cargo.toml`
* `--package <PACKAGE>` — Package to build
* `--profile <PROFILE>` — Build with the specified profile

  Default value: `release`
* `--features <FEATURES>` — Build with the list of features activated, space or comma separated
* `--all-features` — Build with the all features activated
* `--no-default-features` — Build with the default feature not activated
* `--out-dir <OUT_DIR>` — Directory to copy wasm files to
* `--print-commands-only` — Print commands to build without executing them



## `soroban contract bump`

Extend the expiry ledger of a contract-data ledger entry

**Usage:** `soroban contract bump [OPTIONS] --durability <DURABILITY> --ledgers-to-expire <LEDGERS_TO_EXPIRE>`

###### **Options:**

* `--id <CONTRACT_ID>` — Contract ID to which owns the data entries
* `--key <KEY>` — Storage key (symbols only)
* `--key-xdr <KEY_XDR>` — Storage key (base64-encoded XDR)
* `--wasm <WASM>` — Path to Wasm file of contract code to bump
* `--durability <DURABILITY>` — Storage entry durability

  Possible values:
  - `persistent`:
    Persistent
  - `temporary`:
    Temporary

* `--ledgers-to-expire <LEDGERS_TO_EXPIRE>` — Number of ledgers to extend the entries
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--source-account <SOURCE_ACCOUNT>` — Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
* `--hd-path <HD_PATH>` — If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`



## `soroban contract deploy`

Deploy a contract

**Usage:** `soroban contract deploy [OPTIONS] <--wasm <WASM>|--wasm-hash <WASM_HASH>>`

###### **Options:**

* `--wasm <WASM>` — WASM file to deploy
* `--wasm-hash <WASM_HASH>` — Hash of the already installed/deployed WASM file
* `--id <CONTRACT_ID>` — Contract ID to deploy to
* `--salt <SALT>` — Custom salt 32-byte salt for the token id
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--source-account <SOURCE_ACCOUNT>` — Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
* `--hd-path <HD_PATH>` — If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`



## `soroban contract fetch`

Fetch a contract's Wasm binary from a network or local sandbox

**Usage:** `soroban contract fetch [OPTIONS] --id <CONTRACT_ID>`

###### **Options:**

* `--id <CONTRACT_ID>` — Contract ID to fetch
* `-o`, `--out-file <OUT_FILE>` — Where to write output otherwise stdout is used
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`



## `soroban contract inspect`

Inspect a WASM file listing contract functions, meta, etc

**Usage:** `soroban contract inspect [OPTIONS] --wasm <WASM>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban contract install`

Install a WASM file to the ledger without creating a contract instance

**Usage:** `soroban contract install [OPTIONS] --wasm <WASM>`

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--source-account <SOURCE_ACCOUNT>` — Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
* `--hd-path <HD_PATH>` — If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--wasm <WASM>` — Path to wasm binary



## `soroban contract invoke`

Invoke a contract function

Generates an "implicit CLI" for the specified contract on-the-fly using the contract's schema, which gets embedded into every Soroban contract. The "slop" in this command, everything after the `--`, gets passed to this implicit CLI. Get in-depth help for a given contract:

soroban contract invoke ... -- --help

**Usage:** `soroban contract invoke [OPTIONS] --id <CONTRACT_ID> [-- <CONTRACT_FN_AND_ARGS>...]`

###### **Arguments:**

* `<CONTRACT_FN_AND_ARGS>`

###### **Options:**

* `--id <CONTRACT_ID>` — Contract ID to invoke
* `--wasm <WASM>` — WASM file of the contract to invoke (if using sandbox will deploy this file)
* `--cost` — Output the cost execution to stderr
* `--unlimited-budget` — Run with an unlimited budget
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--source-account <SOURCE_ACCOUNT>` — Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
* `--hd-path <HD_PATH>` — If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--events-file <PATH>` — File to persist events, default is `.soroban/events.json`
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`



## `soroban contract optimize`

Optimize a WASM file

**Usage:** `soroban contract optimize [OPTIONS] --wasm <WASM>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary
* `--wasm-out <WASM_OUT>` — Path to write the optimized WASM file to (defaults to same location as --wasm with .optimized.wasm suffix)



## `soroban contract read`

Print the current value of a contract-data ledger entry

**Usage:** `soroban contract read [OPTIONS] --id <CONTRACT_ID>`

###### **Options:**

* `--id <CONTRACT_ID>` — Contract ID to invoke
* `--key <KEY>` — Storage key (symbols only)
* `--key-xdr <KEY_XDR>` — Storage key (base64-encoded XDR ScVal)
* `--durability <DURABILITY>` — Storage entry durability

  Possible values:
  - `persistent`:
    Persistent
  - `temporary`:
    Temporary

* `--output <OUTPUT>` — Type of output to generate

  Default value: `string`

  Possible values:
  - `string`:
    String
  - `json`:
    Json
  - `xdr`:
    XDR

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--source-account <SOURCE_ACCOUNT>` — Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
* `--hd-path <HD_PATH>` — If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban contract restore`

Restore an evicted value for a contract-data legder entry

**Usage:** `soroban contract restore [OPTIONS]`

###### **Options:**

* `--id <CONTRACT_ID>` — Contract ID to which owns the data entries
* `--key <KEY>` — Storage key (symbols only)
* `--key-xdr <KEY_XDR>` — Storage key (base64-encoded XDR)
* `--wasm <WASM>` — Path to Wasm file of contract code to restore
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--source-account <SOURCE_ACCOUNT>` — Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
* `--hd-path <HD_PATH>` — If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`



## `soroban config`

Read and update config

**Usage:** `soroban config <COMMAND>`

###### **Subcommands:**

* `identity` — Configure different identities to sign transactions
* `network` — Configure different networks



## `soroban config identity`

Configure different identities to sign transactions

**Usage:** `soroban config identity <COMMAND>`

###### **Subcommands:**

* `add` — Add a new identity (keypair, ledger, macOS keychain)
* `address` — Given an identity return its address (public key)
* `generate` — Generate a new identity with a seed phrase, currently 12 words
* `ls` — List identities
* `rm` — Remove an identity
* `show` — Given an identity return its private key



## `soroban config identity add`

Add a new identity (keypair, ledger, macOS keychain)

**Usage:** `soroban config identity add [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Name of identity

###### **Options:**

* `--secret-key` — Add using secret_key Can provide with SOROBAN_SECRET_KEY
* `--seed-phrase` — Add using 12 word seed phrase to generate secret_key
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban config identity address`

Given an identity return its address (public key)

**Usage:** `soroban config identity address [OPTIONS] [NAME]`

###### **Arguments:**

* `<NAME>` — Name of identity to lookup, default test identity used if not provided

###### **Options:**

* `--hd-path <HD_PATH>` — If identity is a seed phrase use this hd path, default is 0
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban config identity generate`

Generate a new identity with a seed phrase, currently 12 words

**Usage:** `soroban config identity generate [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Name of identity

###### **Options:**

* `--seed <SEED>` — Optional seed to use when generating seed phrase. Random otherwise
* `-s`, `--as-secret` — Output the generated identity as a secret key
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--hd-path <HD_PATH>` — When generating a secret key, which hd_path should be used from the original seed_phrase
* `-d`, `--default-seed` — Generate the default seed phrase. Useful for testing. Equivalent to --seed 0000000000000000



## `soroban config identity ls`

List identities

**Usage:** `soroban config identity ls [OPTIONS]`

###### **Options:**

* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban config identity rm`

Remove an identity

**Usage:** `soroban config identity rm [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Identity to remove

###### **Options:**

* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban config identity show`

Given an identity return its private key

**Usage:** `soroban config identity show [OPTIONS] [NAME]`

###### **Arguments:**

* `<NAME>` — Name of identity to lookup, default is test identity

###### **Options:**

* `--hd-path <HD_PATH>` — If identity is a seed phrase use this hd path, default is 0
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban config network`

Configure different networks

**Usage:** `soroban config network <COMMAND>`

###### **Subcommands:**

* `add` — Add a new network
* `rm` — Remove a network
* `ls` — List networks



## `soroban config network add`

Add a new network

**Usage:** `soroban config network add [OPTIONS] --rpc-url <RPC_URL> --network-passphrase <NETWORK_PASSPHRASE> <NAME>`

###### **Arguments:**

* `<NAME>` — Name of network

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban config network rm`

Remove a network

**Usage:** `soroban config network rm [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Network to remove

###### **Options:**

* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban config network ls`

List networks

**Usage:** `soroban config network ls [OPTIONS]`

###### **Options:**

* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `-l`, `--long` — Get more info about the networks



## `soroban events`

Watch the network for contract events

**Usage:** `soroban events [OPTIONS]`

###### **Options:**

* `--start-ledger <START_LEDGER>` — The first ledger sequence number in the range to pull events (required if not in sandbox mode). https://developers.stellar.org/docs/encyclopedia/ledger-headers#ledger-sequence
* `--cursor <CURSOR>` — The cursor corresponding to the start of the event range
* `--output <OUTPUT>` — Output formatting options for event stream

  Default value: `pretty`

  Possible values:
  - `pretty`:
    Colorful, human-oriented console output
  - `plain`:
    Human-oriented console output without colors
  - `json`:
    JSONified console output

* `-c`, `--count <COUNT>` — The maximum number of events to display (specify "0" to show all events when using sandbox, or to defer to the server-defined limit if using RPC)

  Default value: `10`
* `--id <CONTRACT_IDS>` — A set of (up to 5) contract IDs to filter events on. This parameter can be passed multiple times, e.g. `--id abc --id def`, or passed with multiple parameters, e.g. `--id abd def`
* `--topic <TOPIC_FILTERS>` — A set of (up to 4) topic filters to filter event topics on. A single topic filter can contain 1-4 different segment filters, separated by commas, with an asterisk (* character) indicating a wildcard segment
* `--type <EVENT_TYPE>` — Specifies which type of contract events to display

  Default value: `all`

  Possible values: `all`, `contract`, `system`

* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--events-file <PATH>` — File to persist events, default is `.soroban/events.json`



## `soroban lab`

Experiment with early features and expert tools

**Usage:** `soroban lab <COMMAND>`

###### **Subcommands:**

* `token` — Wrap, create, and manage token contracts
* `xdr` — Decode xdr



## `soroban lab token`

Wrap, create, and manage token contracts

**Usage:** `soroban lab token <COMMAND>`

###### **Subcommands:**

* `wrap` — Deploy a token contract to wrap an existing Stellar classic asset for smart contract usage
* `id` — Compute the expected contract id for the given asset



## `soroban lab token wrap`

Deploy a token contract to wrap an existing Stellar classic asset for smart contract usage

**Usage:** `soroban lab token wrap [OPTIONS] --asset <ASSET>`

###### **Options:**

* `--asset <ASSET>` — ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--source-account <SOURCE_ACCOUNT>` — Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
* `--hd-path <HD_PATH>` — If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`



## `soroban lab token id`

Compute the expected contract id for the given asset

**Usage:** `soroban lab token id [OPTIONS] --asset <ASSET>`

###### **Options:**

* `--asset <ASSET>` — ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--source-account <SOURCE_ACCOUNT>` — Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
* `--hd-path <HD_PATH>` — If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



## `soroban lab xdr`

Decode xdr

**Usage:** `soroban lab xdr <COMMAND>`

###### **Subcommands:**

* `dec` — Decode XDR



## `soroban lab xdr dec`

Decode XDR

**Usage:** `soroban lab xdr dec [OPTIONS] --type <TYPE> --xdr <XDR>`

###### **Options:**

* `--type <TYPE>` — XDR type to decode to

  Possible values: `Value`, `ScpBallot`, `ScpStatementType`, `ScpNomination`, `ScpStatement`, `ScpStatementPledges`, `ScpStatementPrepare`, `ScpStatementConfirm`, `ScpStatementExternalize`, `ScpEnvelope`, `ScpQuorumSet`, `ConfigSettingContractExecutionLanesV0`, `ConfigSettingContractComputeV0`, `ConfigSettingContractLedgerCostV0`, `ConfigSettingContractHistoricalDataV0`, `ConfigSettingContractMetaDataV0`, `ConfigSettingContractBandwidthV0`, `ContractCostType`, `ContractCostParamEntry`, `StateExpirationSettings`, `ContractCostParams`, `ConfigSettingId`, `ConfigSettingEntry`, `ScEnvMetaKind`, `ScEnvMetaEntry`, `ScMetaV0`, `ScMetaKind`, `ScMetaEntry`, `ScSpecType`, `ScSpecTypeOption`, `ScSpecTypeResult`, `ScSpecTypeVec`, `ScSpecTypeMap`, `ScSpecTypeSet`, `ScSpecTypeTuple`, `ScSpecTypeBytesN`, `ScSpecTypeUdt`, `ScSpecTypeDef`, `ScSpecUdtStructFieldV0`, `ScSpecUdtStructV0`, `ScSpecUdtUnionCaseVoidV0`, `ScSpecUdtUnionCaseTupleV0`, `ScSpecUdtUnionCaseV0Kind`, `ScSpecUdtUnionCaseV0`, `ScSpecUdtUnionV0`, `ScSpecUdtEnumCaseV0`, `ScSpecUdtEnumV0`, `ScSpecUdtErrorEnumCaseV0`, `ScSpecUdtErrorEnumV0`, `ScSpecFunctionInputV0`, `ScSpecFunctionV0`, `ScSpecEntryKind`, `ScSpecEntry`, `ScValType`, `ScErrorType`, `ScErrorCode`, `ScError`, `UInt128Parts`, `Int128Parts`, `UInt256Parts`, `Int256Parts`, `ContractExecutableType`, `ContractExecutable`, `ScAddressType`, `ScAddress`, `ScVec`, `ScMap`, `ScBytes`, `ScString`, `ScSymbol`, `ScNonceKey`, `ScContractInstance`, `ScVal`, `ScMapEntry`, `StoredTransactionSet`, `PersistedScpStateV0`, `PersistedScpStateV1`, `PersistedScpState`, `Thresholds`, `String32`, `String64`, `SequenceNumber`, `DataValue`, `PoolId`, `AssetCode4`, `AssetCode12`, `AssetType`, `AssetCode`, `AlphaNum4`, `AlphaNum12`, `Asset`, `Price`, `Liabilities`, `ThresholdIndexes`, `LedgerEntryType`, `Signer`, `AccountFlags`, `SponsorshipDescriptor`, `AccountEntryExtensionV3`, `AccountEntryExtensionV2`, `AccountEntryExtensionV2Ext`, `AccountEntryExtensionV1`, `AccountEntryExtensionV1Ext`, `AccountEntry`, `AccountEntryExt`, `TrustLineFlags`, `LiquidityPoolType`, `TrustLineAsset`, `TrustLineEntryExtensionV2`, `TrustLineEntryExtensionV2Ext`, `TrustLineEntry`, `TrustLineEntryExt`, `TrustLineEntryV1`, `TrustLineEntryV1Ext`, `OfferEntryFlags`, `OfferEntry`, `OfferEntryExt`, `DataEntry`, `DataEntryExt`, `ClaimPredicateType`, `ClaimPredicate`, `ClaimantType`, `Claimant`, `ClaimantV0`, `ClaimableBalanceIdType`, `ClaimableBalanceId`, `ClaimableBalanceFlags`, `ClaimableBalanceEntryExtensionV1`, `ClaimableBalanceEntryExtensionV1Ext`, `ClaimableBalanceEntry`, `ClaimableBalanceEntryExt`, `LiquidityPoolConstantProductParameters`, `LiquidityPoolEntry`, `LiquidityPoolEntryBody`, `LiquidityPoolEntryConstantProduct`, `ContractEntryBodyType`, `ContractDataFlags`, `ContractDataDurability`, `ContractDataEntry`, `ContractDataEntryBody`, `ContractDataEntryData`, `ContractCodeEntry`, `ContractCodeEntryBody`, `LedgerEntryExtensionV1`, `LedgerEntryExtensionV1Ext`, `LedgerEntry`, `LedgerEntryData`, `LedgerEntryExt`, `LedgerKey`, `LedgerKeyAccount`, `LedgerKeyTrustLine`, `LedgerKeyOffer`, `LedgerKeyData`, `LedgerKeyClaimableBalance`, `LedgerKeyLiquidityPool`, `LedgerKeyContractData`, `LedgerKeyContractCode`, `LedgerKeyConfigSetting`, `EnvelopeType`, `UpgradeType`, `StellarValueType`, `LedgerCloseValueSignature`, `StellarValue`, `StellarValueExt`, `LedgerHeaderFlags`, `LedgerHeaderExtensionV1`, `LedgerHeaderExtensionV1Ext`, `LedgerHeader`, `LedgerHeaderExt`, `LedgerUpgradeType`, `ConfigUpgradeSetKey`, `LedgerUpgrade`, `ConfigUpgradeSet`, `BucketEntryType`, `BucketMetadata`, `BucketMetadataExt`, `BucketEntry`, `TxSetComponentType`, `TxSetComponent`, `TxSetComponentTxsMaybeDiscountedFee`, `TransactionPhase`, `TransactionSet`, `TransactionSetV1`, `GeneralizedTransactionSet`, `TransactionResultPair`, `TransactionResultSet`, `TransactionHistoryEntry`, `TransactionHistoryEntryExt`, `TransactionHistoryResultEntry`, `TransactionHistoryResultEntryExt`, `LedgerHeaderHistoryEntry`, `LedgerHeaderHistoryEntryExt`, `LedgerScpMessages`, `ScpHistoryEntryV0`, `ScpHistoryEntry`, `LedgerEntryChangeType`, `LedgerEntryChange`, `LedgerEntryChanges`, `OperationMeta`, `TransactionMetaV1`, `TransactionMetaV2`, `ContractEventType`, `ContractEvent`, `ContractEventBody`, `ContractEventV0`, `DiagnosticEvent`, `SorobanTransactionMeta`, `TransactionMetaV3`, `InvokeHostFunctionSuccessPreImage`, `TransactionMeta`, `TransactionResultMeta`, `UpgradeEntryMeta`, `LedgerCloseMetaV0`, `LedgerCloseMetaV1`, `LedgerCloseMetaV2`, `LedgerCloseMeta`, `ErrorCode`, `SError`, `SendMore`, `SendMoreExtended`, `AuthCert`, `Hello`, `Auth`, `IpAddrType`, `PeerAddress`, `PeerAddressIp`, `MessageType`, `DontHave`, `SurveyMessageCommandType`, `SurveyMessageResponseType`, `SurveyRequestMessage`, `SignedSurveyRequestMessage`, `EncryptedBody`, `SurveyResponseMessage`, `SignedSurveyResponseMessage`, `PeerStats`, `PeerStatList`, `TopologyResponseBodyV0`, `TopologyResponseBodyV1`, `SurveyResponseBody`, `TxAdvertVector`, `FloodAdvert`, `TxDemandVector`, `FloodDemand`, `StellarMessage`, `AuthenticatedMessage`, `AuthenticatedMessageV0`, `LiquidityPoolParameters`, `MuxedAccount`, `MuxedAccountMed25519`, `DecoratedSignature`, `OperationType`, `CreateAccountOp`, `PaymentOp`, `PathPaymentStrictReceiveOp`, `PathPaymentStrictSendOp`, `ManageSellOfferOp`, `ManageBuyOfferOp`, `CreatePassiveSellOfferOp`, `SetOptionsOp`, `ChangeTrustAsset`, `ChangeTrustOp`, `AllowTrustOp`, `ManageDataOp`, `BumpSequenceOp`, `CreateClaimableBalanceOp`, `ClaimClaimableBalanceOp`, `BeginSponsoringFutureReservesOp`, `RevokeSponsorshipType`, `RevokeSponsorshipOp`, `RevokeSponsorshipOpSigner`, `ClawbackOp`, `ClawbackClaimableBalanceOp`, `SetTrustLineFlagsOp`, `LiquidityPoolDepositOp`, `LiquidityPoolWithdrawOp`, `HostFunctionType`, `ContractIdPreimageType`, `ContractIdPreimage`, `ContractIdPreimageFromAddress`, `CreateContractArgs`, `HostFunction`, `SorobanAuthorizedFunctionType`, `SorobanAuthorizedContractFunction`, `SorobanAuthorizedFunction`, `SorobanAuthorizedInvocation`, `SorobanAddressCredentials`, `SorobanCredentialsType`, `SorobanCredentials`, `SorobanAuthorizationEntry`, `InvokeHostFunctionOp`, `BumpFootprintExpirationOp`, `RestoreFootprintOp`, `Operation`, `OperationBody`, `HashIdPreimage`, `HashIdPreimageOperationId`, `HashIdPreimageRevokeId`, `HashIdPreimageContractId`, `HashIdPreimageSorobanAuthorization`, `MemoType`, `Memo`, `TimeBounds`, `LedgerBounds`, `PreconditionsV2`, `PreconditionType`, `Preconditions`, `LedgerFootprint`, `SorobanResources`, `SorobanTransactionData`, `TransactionV0`, `TransactionV0Ext`, `TransactionV0Envelope`, `Transaction`, `TransactionExt`, `TransactionV1Envelope`, `FeeBumpTransaction`, `FeeBumpTransactionInnerTx`, `FeeBumpTransactionExt`, `FeeBumpTransactionEnvelope`, `TransactionEnvelope`, `TransactionSignaturePayload`, `TransactionSignaturePayloadTaggedTransaction`, `ClaimAtomType`, `ClaimOfferAtomV0`, `ClaimOfferAtom`, `ClaimLiquidityAtom`, `ClaimAtom`, `CreateAccountResultCode`, `CreateAccountResult`, `PaymentResultCode`, `PaymentResult`, `PathPaymentStrictReceiveResultCode`, `SimplePaymentResult`, `PathPaymentStrictReceiveResult`, `PathPaymentStrictReceiveResultSuccess`, `PathPaymentStrictSendResultCode`, `PathPaymentStrictSendResult`, `PathPaymentStrictSendResultSuccess`, `ManageSellOfferResultCode`, `ManageOfferEffect`, `ManageOfferSuccessResult`, `ManageOfferSuccessResultOffer`, `ManageSellOfferResult`, `ManageBuyOfferResultCode`, `ManageBuyOfferResult`, `SetOptionsResultCode`, `SetOptionsResult`, `ChangeTrustResultCode`, `ChangeTrustResult`, `AllowTrustResultCode`, `AllowTrustResult`, `AccountMergeResultCode`, `AccountMergeResult`, `InflationResultCode`, `InflationPayout`, `InflationResult`, `ManageDataResultCode`, `ManageDataResult`, `BumpSequenceResultCode`, `BumpSequenceResult`, `CreateClaimableBalanceResultCode`, `CreateClaimableBalanceResult`, `ClaimClaimableBalanceResultCode`, `ClaimClaimableBalanceResult`, `BeginSponsoringFutureReservesResultCode`, `BeginSponsoringFutureReservesResult`, `EndSponsoringFutureReservesResultCode`, `EndSponsoringFutureReservesResult`, `RevokeSponsorshipResultCode`, `RevokeSponsorshipResult`, `ClawbackResultCode`, `ClawbackResult`, `ClawbackClaimableBalanceResultCode`, `ClawbackClaimableBalanceResult`, `SetTrustLineFlagsResultCode`, `SetTrustLineFlagsResult`, `LiquidityPoolDepositResultCode`, `LiquidityPoolDepositResult`, `LiquidityPoolWithdrawResultCode`, `LiquidityPoolWithdrawResult`, `InvokeHostFunctionResultCode`, `InvokeHostFunctionResult`, `BumpFootprintExpirationResultCode`, `BumpFootprintExpirationResult`, `RestoreFootprintResultCode`, `RestoreFootprintResult`, `OperationResultCode`, `OperationResult`, `OperationResultTr`, `TransactionResultCode`, `InnerTransactionResult`, `InnerTransactionResultResult`, `InnerTransactionResultExt`, `InnerTransactionResultPair`, `TransactionResult`, `TransactionResultResult`, `TransactionResultExt`, `Hash`, `Uint256`, `Uint32`, `Int32`, `Uint64`, `Int64`, `TimePoint`, `Duration`, `ExtensionPoint`, `CryptoKeyType`, `PublicKeyType`, `SignerKeyType`, `PublicKey`, `SignerKey`, `SignerKeyEd25519SignedPayload`, `Signature`, `SignatureHint`, `NodeId`, `AccountId`, `Curve25519Secret`, `Curve25519Public`, `HmacSha256Key`, `HmacSha256Mac`

* `--xdr <XDR>` — XDR (base64 encoded) to decode
* `--output <OUTPUT>` — Type of output

  Default value: `default`

  Possible values:
  - `default`
  - `json`:
    Json representation




## `soroban version`

Print version information

**Usage:** `soroban version`



## `soroban completion`

Print shell completion code for the specified shell

Ensure the completion package for your shell is installed,
e.g., bash-completion for bash.

To enable autocomplete in the current bash shell, run:
  source <(soroban completion --shell bash)

To enable autocomplete permanently, run:
  echo "source <(soroban completion --shell bash)" >> ~/.bashrc

**Usage:** `soroban completion --shell <SHELL>`

###### **Options:**

* `--shell <SHELL>` — The shell type

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
