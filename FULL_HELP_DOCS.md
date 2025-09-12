# Stellar CLI Manual

This document contains the help content for the `stellar` command-line program.

## `stellar`

Work seamlessly with Stellar accounts, contracts, and assets from the command line.

- Generate and manage keys and accounts
- Build, deploy, and interact with contracts
- Deploy asset contracts
- Stream events
- Start local testnets
- Decode, encode XDR
- More!

For additional information see:

- Stellar Docs: https://developers.stellar.org
- Smart Contract Docs: https://developers.stellar.org/docs/build/smart-contracts/overview
- CLI Docs: https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli

To get started generate a new identity:

    stellar keys generate alice

Use keys with the `--source` flag in other commands.

Commands that work with contracts are organized under the `contract` subcommand. List them:

    stellar contract --help

Use contracts like a CLI:

    stellar contract invoke --id CCR6QKTWZQYW6YUJ7UP7XXZRLWQPFRV6SWBLQS4ZQOSAF4BOUD77OTE2 --source alice --network testnet -- --help

Anything after the `--` double dash (the "slop") is parsed as arguments to the contract-specific CLI, generated on-the-fly from the contract schema. For the hello world example, with a function called `hello` that takes one string argument `to`, here's how you invoke it:

    stellar contract invoke --id CCR6QKTWZQYW6YUJ7UP7XXZRLWQPFRV6SWBLQS4ZQOSAF4BOUD77OTE2 --source alice --network testnet -- hello --to world


**Usage:** `stellar [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `contract` — Tools for smart contract developers
* `doctor` — Diagnose and troubleshoot CLI and network issues
* `events` — Watch the network for contract events
* `env` — Prints the environment variables
* `keys` — Create and manage identities including keys and addresses
* `network` — Configure connection to networks
* `container` — Start local networks in containers
* `config` — Manage cli configuration
* `snapshot` — Download a snapshot of a ledger from an archive
* `tx` — Sign, Simulate, and Send transactions
* `xdr` — Decode and encode XDR
* `completion` — Print shell completion code for the specified shell
* `cache` — Cache for transactions and contract specs
* `version` — Print version information
* `plugin` — The subcommand for CLI plugins
* `ledger` — Fetch ledger information
* `fee-stats` — Fetch network feestats

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `-f`, `--filter-logs <FILTER_LOGS>` — Filter logs output. To turn on `stellar_cli::log::footprint=debug` or off `=off`. Can also use env var `RUST_LOG`
* `-q`, `--quiet` — Do not write logs to stderr including `INFO`
* `-v`, `--verbose` — Log DEBUG events
* `--very-verbose` [alias: `vv`] — Log DEBUG and TRACE events
* `--list` — List installed plugins. E.g. `stellar-hello`
* `--no-cache` — Do not cache your simulations and transactions



## `stellar contract`

Tools for smart contract developers

**Usage:** `stellar contract <COMMAND>`

###### **Subcommands:**

* `asset` — Utilities to deploy a Stellar Asset Contract or get its id
* `alias` — Utilities to manage contract aliases
* `bindings` — Generate code client bindings for a contract
* `build` — Build a contract from source
* `extend` — Extend the time to live ledger of a contract-data ledger entry
* `deploy` — Deploy a wasm contract
* `fetch` — Fetch a contract's Wasm binary
* `id` — Generate the contract id for a given contract or asset
* `info` — Access info about contracts
* `init` — Initialize a Soroban contract project
* `inspect` — (Deprecated in favor of `contract info` subcommand) Inspect a WASM file listing contract functions, meta, etc
* `upload` — Install a WASM file to the ledger without creating a contract instance
* `install` — (Deprecated in favor of `contract upload` subcommand) Install a WASM file to the ledger without creating a contract instance
* `invoke` — Invoke a contract function
* `optimize` — Optimize a WASM file
* `read` — Print the current value of a contract-data ledger entry
* `restore` — Restore an evicted value for a contract-data legder entry



## `stellar contract asset`

Utilities to deploy a Stellar Asset Contract or get its id

**Usage:** `stellar contract asset <COMMAND>`

###### **Subcommands:**

* `id` — Get Id of builtin Soroban Asset Contract. Deprecated, use `stellar contract id asset` instead
* `deploy` — Deploy builtin Soroban Asset Contract



## `stellar contract asset id`

Get Id of builtin Soroban Asset Contract. Deprecated, use `stellar contract id asset` instead

**Usage:** `stellar contract asset id [OPTIONS] --asset <ASSET>`

###### **Options:**

* `--asset <ASSET>` — ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar contract asset deploy`

Deploy builtin Soroban Asset Contract

**Usage:** `stellar contract asset deploy [OPTIONS] --asset <ASSET> --source-account <SOURCE_ACCOUNT>`

###### **Options:**

* `--asset <ASSET>` — ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--alias <ALIAS>` — The alias that will be used to save the assets's id. Whenever used, `--alias` will always overwrite the existing contract id configuration without asking for confirmation



## `stellar contract alias`

Utilities to manage contract aliases

**Usage:** `stellar contract alias <COMMAND>`

###### **Subcommands:**

* `remove` — Remove contract alias
* `add` — Add contract alias
* `show` — Show the contract id associated with a given alias
* `ls` — List all aliases



## `stellar contract alias remove`

Remove contract alias

**Usage:** `stellar contract alias remove [OPTIONS] <ALIAS>`

###### **Arguments:**

* `<ALIAS>` — The contract alias that will be removed

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config



## `stellar contract alias add`

Add contract alias

**Usage:** `stellar contract alias add [OPTIONS] --id <CONTRACT_ID> <ALIAS>`

###### **Arguments:**

* `<ALIAS>` — The contract alias that will be used

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--overwrite` — Overwrite the contract alias if it already exists
* `--id <CONTRACT_ID>` — The contract id that will be associated with the alias



## `stellar contract alias show`

Show the contract id associated with a given alias

**Usage:** `stellar contract alias show [OPTIONS] <ALIAS>`

###### **Arguments:**

* `<ALIAS>` — The contract alias that will be displayed

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config



## `stellar contract alias ls`

List all aliases

**Usage:** `stellar contract alias ls [OPTIONS]`

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar contract bindings`

Generate code client bindings for a contract

**Usage:** `stellar contract bindings <COMMAND>`

###### **Subcommands:**

* `json` — Generate Json Bindings
* `rust` — Generate Rust bindings
* `typescript` — Generate a TypeScript / JavaScript package
* `python` — Generate Python bindings
* `java` — Generate Java bindings
* `flutter` — Generate Flutter bindings
* `swift` — Generate Swift bindings
* `php` — Generate PHP bindings



## `stellar contract bindings json`

Generate Json Bindings

**Usage:** `stellar contract bindings json --wasm <WASM>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary



## `stellar contract bindings rust`

Generate Rust bindings

**Usage:** `stellar contract bindings rust --wasm <WASM>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary



## `stellar contract bindings typescript`

Generate a TypeScript / JavaScript package

**Usage:** `stellar contract bindings typescript [OPTIONS] --output-dir <OUTPUT_DIR> <--wasm <WASM>|--wasm-hash <WASM_HASH>|--contract-id <CONTRACT_ID>>`

###### **Options:**

* `--wasm <WASM>` — Wasm file path on local filesystem. Provide this OR `--wasm-hash` OR `--contract-id`
* `--wasm-hash <WASM_HASH>` — Hash of Wasm blob on a network. Provide this OR `--wasm` OR `--contract-id`
* `--contract-id <CONTRACT_ID>` [alias: `id`] — Contract ID/alias on a network. Provide this OR `--wasm-hash` OR `--wasm`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--output-dir <OUTPUT_DIR>` — Where to place generated project
* `--overwrite` — Whether to overwrite output directory if it already exists



## `stellar contract bindings python`

Generate Python bindings

**Usage:** `stellar contract bindings python`



## `stellar contract bindings java`

Generate Java bindings

**Usage:** `stellar contract bindings java`



## `stellar contract bindings flutter`

Generate Flutter bindings

**Usage:** `stellar contract bindings flutter`



## `stellar contract bindings swift`

Generate Swift bindings

**Usage:** `stellar contract bindings swift`



## `stellar contract bindings php`

Generate PHP bindings

**Usage:** `stellar contract bindings php`



## `stellar contract build`

Build a contract from source

Builds all crates that are referenced by the cargo manifest (Cargo.toml) that have cdylib as their crate-type. Crates are built for the wasm32 target. Unless configured otherwise, crates are built with their default features and with their release profile.

In workspaces builds all crates unless a package name is specified, or the command is executed from the sub-directory of a workspace crate.

To view the commands that will be executed, without executing them, use the --print-commands-only option.

**Usage:** `stellar contract build [OPTIONS]`

###### **Options:**

* `--manifest-path <MANIFEST_PATH>` — Path to Cargo.toml
* `--package <PACKAGE>` — Package to build

   If omitted, all packages that build for crate-type cdylib are built.
* `--profile <PROFILE>` — Build with the specified profile

  Default value: `release`
* `--features <FEATURES>` — Build with the list of features activated, space or comma separated
* `--all-features` — Build with the all features activated
* `--no-default-features` — Build with the default feature not activated
* `--out-dir <OUT_DIR>` — Directory to copy wasm files to

   If provided, wasm files can be found in the cargo target directory, and the specified directory.

   If ommitted, wasm files are written only to the cargo target directory.
* `--print-commands-only` — Print commands to build without executing them
* `--meta <META>` — Add key-value to contract meta (adds the meta to the `contractmetav0` custom section)



## `stellar contract extend`

Extend the time to live ledger of a contract-data ledger entry.

If no keys are specified the contract itself is extended.

**Usage:** `stellar contract extend [OPTIONS] --ledgers-to-extend <LEDGERS_TO_EXTEND> --source-account <SOURCE_ACCOUNT>`

###### **Options:**

* `--ledgers-to-extend <LEDGERS_TO_EXTEND>` — Number of ledgers to extend the entries
* `--ttl-ledger-only` — Only print the new Time To Live ledger
* `--id <CONTRACT_ID>` — Contract ID to which owns the data entries. If no keys provided the Contract's instance will be extended
* `--key <KEY>` — Storage key (symbols only)
* `--key-xdr <KEY_XDR>` — Storage key (base64-encoded XDR)
* `--wasm <WASM>` — Path to Wasm file of contract code to extend
* `--wasm-hash <WASM_HASH>` — Path to Wasm file of contract code to extend
* `--durability <DURABILITY>` — Storage entry durability

  Default value: `persistent`

  Possible values:
  - `persistent`:
    Persistent
  - `temporary`:
    Temporary

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout



## `stellar contract deploy`

Deploy a wasm contract

**Usage:** `stellar contract deploy [OPTIONS] --source-account <SOURCE_ACCOUNT> <--wasm <WASM>|--wasm-hash <WASM_HASH>> [-- <CONTRACT_CONSTRUCTOR_ARGS>...]`

###### **Arguments:**

* `<CONTRACT_CONSTRUCTOR_ARGS>` — If provided, will be passed to the contract's `__constructor` function with provided arguments for that function as `--arg-name value`

###### **Options:**

* `--wasm <WASM>` — WASM file to deploy
* `--wasm-hash <WASM_HASH>` — Hash of the already installed/deployed WASM file
* `--salt <SALT>` — Custom salt 32-byte salt for the token id
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `-i`, `--ignore-checks` — Whether to ignore safety checks when deploying contracts

  Default value: `false`
* `--alias <ALIAS>` — The alias that will be used to save the contract's id. Whenever used, `--alias` will always overwrite the existing contract id configuration without asking for confirmation



## `stellar contract fetch`

Fetch a contract's Wasm binary

**Usage:** `stellar contract fetch [OPTIONS] --id <CONTRACT_ID>`

###### **Options:**

* `--id <CONTRACT_ID>` — Contract ID to fetch
* `-o`, `--out-file <OUT_FILE>` — Where to write output otherwise stdout is used
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config



## `stellar contract id`

Generate the contract id for a given contract or asset

**Usage:** `stellar contract id <COMMAND>`

###### **Subcommands:**

* `asset` — Deploy builtin Soroban Asset Contract
* `wasm` — Deploy normal Wasm Contract



## `stellar contract id asset`

Deploy builtin Soroban Asset Contract

**Usage:** `stellar contract id asset [OPTIONS] --asset <ASSET>`

###### **Options:**

* `--asset <ASSET>` — ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar contract id wasm`

Deploy normal Wasm Contract

**Usage:** `stellar contract id wasm [OPTIONS] --salt <SALT> --source-account <SOURCE_ACCOUNT>`

###### **Options:**

* `--salt <SALT>` — ID of the Soroban contract
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet



## `stellar contract info`

Access info about contracts

**Usage:** `stellar contract info <COMMAND>`

###### **Subcommands:**

* `interface` — Output the interface of a contract
* `meta` — Output the metadata stored in a contract
* `env-meta` — Output the env required metadata stored in a contract
* `build` — Output the contract build information, if available



## `stellar contract info interface`

Output the interface of a contract.

A contract's interface describes the functions, parameters, and types that the contract makes accessible to be called.

The data outputted by this command is a stream of `SCSpecEntry` XDR values. See the type definitions in [stellar-xdr](https://github.com/stellar/stellar-xdr). [See also XDR data format](https://developers.stellar.org/docs/learn/encyclopedia/data-format/xdr).

Outputs no data when no data is present in the contract.

**Usage:** `stellar contract info interface [OPTIONS] <--wasm <WASM>|--wasm-hash <WASM_HASH>|--contract-id <CONTRACT_ID>>`

###### **Options:**

* `--wasm <WASM>` — Wasm file path on local filesystem. Provide this OR `--wasm-hash` OR `--contract-id`
* `--wasm-hash <WASM_HASH>` — Hash of Wasm blob on a network. Provide this OR `--wasm` OR `--contract-id`
* `--contract-id <CONTRACT_ID>` [alias: `id`] — Contract ID/alias on a network. Provide this OR `--wasm-hash` OR `--wasm`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--output <OUTPUT>` — Format of the output

  Default value: `rust`

  Possible values:
  - `rust`:
    Rust code output of the contract interface
  - `xdr-base64`:
    XDR output of the info entry
  - `json`:
    JSON output of the info entry (one line, not formatted)
  - `json-formatted`:
    Formatted (multiline) JSON output of the info entry




## `stellar contract info meta`

Output the metadata stored in a contract.

A contract's meta is a series of key-value pairs that the contract developer can set with any values to provided metadata about the contract. The meta also contains some information like the version of Rust SDK, and Rust compiler version.

The data outputted by this command is a stream of `SCMetaEntry` XDR values. See the type definitions in [stellar-xdr](https://github.com/stellar/stellar-xdr). [See also XDR data format](https://developers.stellar.org/docs/learn/encyclopedia/data-format/xdr).

Outputs no data when no data is present in the contract.

**Usage:** `stellar contract info meta [OPTIONS] <--wasm <WASM>|--wasm-hash <WASM_HASH>|--contract-id <CONTRACT_ID>>`

###### **Options:**

* `--wasm <WASM>` — Wasm file path on local filesystem. Provide this OR `--wasm-hash` OR `--contract-id`
* `--wasm-hash <WASM_HASH>` — Hash of Wasm blob on a network. Provide this OR `--wasm` OR `--contract-id`
* `--contract-id <CONTRACT_ID>` [alias: `id`] — Contract ID/alias on a network. Provide this OR `--wasm-hash` OR `--wasm`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--output <OUTPUT>` — Format of the output

  Default value: `text`

  Possible values:
  - `text`:
    Text output of the meta info entry
  - `xdr-base64`:
    XDR output of the info entry
  - `json`:
    JSON output of the info entry (one line, not formatted)
  - `json-formatted`:
    Formatted (multiline) JSON output of the info entry




## `stellar contract info env-meta`

Output the env required metadata stored in a contract.

Env-meta is information stored in all contracts, in the `contractenvmetav0` WASM custom section, about the environment that the contract was built for. Env-meta allows the Soroban Env to know whether the contract is compatibility with the network in its current configuration.

The data outputted by this command is a stream of `SCEnvMetaEntry` XDR values. See the type definitions in [stellar-xdr](https://github.com/stellar/stellar-xdr). [See also XDR data format](https://developers.stellar.org/docs/learn/encyclopedia/data-format/xdr).

Outputs no data when no data is present in the contract.

**Usage:** `stellar contract info env-meta [OPTIONS] <--wasm <WASM>|--wasm-hash <WASM_HASH>|--contract-id <CONTRACT_ID>>`

###### **Options:**

* `--wasm <WASM>` — Wasm file path on local filesystem. Provide this OR `--wasm-hash` OR `--contract-id`
* `--wasm-hash <WASM_HASH>` — Hash of Wasm blob on a network. Provide this OR `--wasm` OR `--contract-id`
* `--contract-id <CONTRACT_ID>` [alias: `id`] — Contract ID/alias on a network. Provide this OR `--wasm-hash` OR `--wasm`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--output <OUTPUT>` — Format of the output

  Default value: `text`

  Possible values:
  - `text`:
    Text output of the meta info entry
  - `xdr-base64`:
    XDR output of the info entry
  - `json`:
    JSON output of the info entry (one line, not formatted)
  - `json-formatted`:
    Formatted (multiline) JSON output of the info entry




## `stellar contract info build`

Output the contract build information, if available.

If the contract has a meta entry like `source_repo=github:user/repo`, this command will try to fetch the attestation information for the WASM file.

**Usage:** `stellar contract info build [OPTIONS] <--wasm <WASM>|--wasm-hash <WASM_HASH>|--contract-id <CONTRACT_ID>>`

###### **Options:**

* `--wasm <WASM>` — Wasm file path on local filesystem. Provide this OR `--wasm-hash` OR `--contract-id`
* `--wasm-hash <WASM_HASH>` — Hash of Wasm blob on a network. Provide this OR `--wasm` OR `--contract-id`
* `--contract-id <CONTRACT_ID>` [alias: `id`] — Contract ID/alias on a network. Provide this OR `--wasm-hash` OR `--wasm`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar contract init`

Initialize a Soroban contract project.

This command will create a Cargo workspace project and add a sample Stellar contract. The name of the contract can be specified by `--name`. It can be run multiple times with different names in order to generate multiple contracts, and files won't be overwritten unless `--overwrite` is passed.

**Usage:** `stellar contract init [OPTIONS] <PROJECT_PATH>`

###### **Arguments:**

* `<PROJECT_PATH>`

###### **Options:**

* `--name <NAME>` — An optional flag to specify a new contract's name.

  Default value: `hello-world`
* `--overwrite` — Overwrite all existing files.



## `stellar contract inspect`

(Deprecated in favor of `contract info` subcommand) Inspect a WASM file listing contract functions, meta, etc

**Usage:** `stellar contract inspect [OPTIONS] --wasm <WASM>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary
* `--output <OUTPUT>` — Output just XDR in base64

  Default value: `docs`

  Possible values:
  - `xdr-base64`:
    XDR of array of contract spec entries
  - `xdr-base64-array`:
    Array of xdr of contract spec entries
  - `docs`:
    Pretty print of contract spec entries

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar contract upload`

Install a WASM file to the ledger without creating a contract instance

**Usage:** `stellar contract upload [OPTIONS] --source-account <SOURCE_ACCOUNT> --wasm <WASM>`

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--wasm <WASM>` — Path to wasm binary
* `-i`, `--ignore-checks` — Whether to ignore safety checks when deploying contracts

  Default value: `false`



## `stellar contract install`

(Deprecated in favor of `contract upload` subcommand) Install a WASM file to the ledger without creating a contract instance

**Usage:** `stellar contract install [OPTIONS] --source-account <SOURCE_ACCOUNT> --wasm <WASM>`

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--wasm <WASM>` — Path to wasm binary
* `-i`, `--ignore-checks` — Whether to ignore safety checks when deploying contracts

  Default value: `false`



## `stellar contract invoke`

Invoke a contract function

Generates an "implicit CLI" for the specified contract on-the-fly using the contract's schema, which gets embedded into every Soroban contract. The "slop" in this command, everything after the `--`, gets passed to this implicit CLI. Get in-depth help for a given contract:

stellar contract invoke ... -- --help

**Usage:** `stellar contract invoke [OPTIONS] --id <CONTRACT_ID> --source-account <SOURCE_ACCOUNT> [-- <CONTRACT_FN_AND_ARGS>...]`

###### **Arguments:**

* `<CONTRACT_FN_AND_ARGS>` — Function name as subcommand, then arguments for that function as `--arg-name value`

###### **Options:**

* `--id <CONTRACT_ID>` — Contract ID to invoke
* `--is-view` — View the result simulating and do not sign and submit transaction. Deprecated use `--send=no`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--send <SEND>` — Whether or not to send a transaction

  Default value: `default`

  Possible values:
  - `default`:
    Send transaction if simulation indicates there are ledger writes, published events, or auth required, otherwise return simulation result
  - `no`:
    Do not send transaction, return simulation result
  - `yes`:
    Always send transaction




## `stellar contract optimize`

Optimize a WASM file

**Usage:** `stellar contract optimize [OPTIONS] --wasm <WASM>...`

###### **Options:**

* `--wasm <WASM>` — Path to one or more wasm binaries
* `--wasm-out <WASM_OUT>` — Path to write the optimized WASM file to (defaults to same location as --wasm with .optimized.wasm suffix)



## `stellar contract read`

Print the current value of a contract-data ledger entry

**Usage:** `stellar contract read [OPTIONS]`

###### **Options:**

* `--output <OUTPUT>` — Type of output to generate

  Default value: `string`

  Possible values:
  - `string`:
    String
  - `json`:
    Json
  - `xdr`:
    XDR

* `--id <CONTRACT_ID>` — Contract ID to which owns the data entries. If no keys provided the Contract's instance will be extended
* `--key <KEY>` — Storage key (symbols only)
* `--key-xdr <KEY_XDR>` — Storage key (base64-encoded XDR)
* `--wasm <WASM>` — Path to Wasm file of contract code to extend
* `--wasm-hash <WASM_HASH>` — Path to Wasm file of contract code to extend
* `--durability <DURABILITY>` — Storage entry durability

  Default value: `persistent`

  Possible values:
  - `persistent`:
    Persistent
  - `temporary`:
    Temporary

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar contract restore`

Restore an evicted value for a contract-data legder entry.

If no keys are specificed the contract itself is restored.

**Usage:** `stellar contract restore [OPTIONS] --source-account <SOURCE_ACCOUNT>`

###### **Options:**

* `--id <CONTRACT_ID>` — Contract ID to which owns the data entries. If no keys provided the Contract's instance will be extended
* `--key <KEY>` — Storage key (symbols only)
* `--key-xdr <KEY_XDR>` — Storage key (base64-encoded XDR)
* `--wasm <WASM>` — Path to Wasm file of contract code to extend
* `--wasm-hash <WASM_HASH>` — Path to Wasm file of contract code to extend
* `--durability <DURABILITY>` — Storage entry durability

  Default value: `persistent`

  Possible values:
  - `persistent`:
    Persistent
  - `temporary`:
    Temporary

* `--ledgers-to-extend <LEDGERS_TO_EXTEND>` — Number of ledgers to extend the entry
* `--ttl-ledger-only` — Only print the new Time To Live ledger
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout



## `stellar doctor`

Diagnose and troubleshoot CLI and network issues

**Usage:** `stellar doctor [OPTIONS]`

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar events`

Watch the network for contract events

**Usage:** `stellar events [OPTIONS]`

###### **Options:**

* `--start-ledger <START_LEDGER>` — The first ledger sequence number in the range to pull events https://developers.stellar.org/docs/learn/encyclopedia/network-configuration/ledger-headers#ledger-sequence
* `--cursor <CURSOR>` — The cursor corresponding to the start of the event range
* `--output <OUTPUT>` — Output formatting options for event stream

  Default value: `pretty`

  Possible values:
  - `pretty`:
    Colorful, human-oriented console output
  - `plain`:
    Human-oriented console output without colors
  - `json`:
    JSON formatted console output

* `-c`, `--count <COUNT>` — The maximum number of events to display (defer to the server-defined limit)

  Default value: `10`
* `--id <CONTRACT_IDS>` — A set of (up to 5) contract IDs to filter events on. This parameter can be passed multiple times, e.g. `--id C123.. --id C456..`, or passed with multiple parameters, e.g. `--id C123 C456`.

   Though the specification supports multiple filter objects (i.e. combinations of type, IDs, and topics), only one set can be specified on the command-line today, though that set can have multiple IDs/topics.
* `--topic <TOPIC_FILTERS>` — A set of (up to 4) topic filters to filter event topics on. A single topic filter can contain 1-4 different segment filters, separated by commas, with an asterisk (`*` character) indicating a wildcard segment.

   **Example:** topic filter with two segments: `--topic "AAAABQAAAAdDT1VOVEVSAA==,*"`

   **Example:** two topic filters with one and two segments each: `--topic "AAAABQAAAAdDT1VOVEVSAA==" --topic '*,*'`

   Note that all of these topic filters are combined with the contract IDs into a single filter (i.e. combination of type, IDs, and topics).
* `--type <EVENT_TYPE>` — Specifies which type of contract events to display

  Default value: `all`

  Possible values: `all`, `contract`, `system`

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config



## `stellar env`

Prints the environment variables

Prints to stdout in a format that can be used as .env file. Environment variables have precedence over defaults.

Pass a name to get the value of a single environment variable.

If there are no environment variables in use, prints the defaults.

**Usage:** `stellar env [OPTIONS] [NAME]`

###### **Arguments:**

* `<NAME>` — Env variable name to get the value of.

   E.g.: $ stellar env STELLAR_ACCOUNT

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar keys`

Create and manage identities including keys and addresses

**Usage:** `stellar keys <COMMAND>`

###### **Subcommands:**

* `add` — Add a new identity (keypair, ledger, OS specific secure store)
* `public-key` — Given an identity return its address (public key)
* `fund` — Fund an identity on a test network
* `generate` — Generate a new identity using a 24-word seed phrase The seed phrase can be stored in a config file (default) or in an OS-specific secure store
* `ls` — List identities
* `rm` — Remove an identity
* `secret` — Output an identity's secret key
* `use` — Set the default identity that will be used on all commands. This allows you to skip `--source-account` or setting a environment variable, while reusing this value in all commands that require it



## `stellar keys add`

Add a new identity (keypair, ledger, OS specific secure store)

**Usage:** `stellar keys add [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Name of identity

###### **Options:**

* `--secret-key` — (deprecated) Enter secret (S) key when prompted
* `--seed-phrase` — (deprecated) Enter key using 12-24 word seed phrase
* `--secure-store` — Save the new key in your OS's credential secure store.

   On Mac this uses Keychain, on Windows it is Secure Store Service, and on *nix platforms it uses a combination of the kernel keyutils and DBus-based Secret Service.

   This only supports seed phrases for now.
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--public-key <PUBLIC_KEY>` — Add a public key, ed25519, or muxed account, e.g. G1.., M2..



## `stellar keys public-key`

Given an identity return its address (public key)

**Usage:** `stellar keys public-key [OPTIONS] <NAME>`

**Command Alias:** `address`

###### **Arguments:**

* `<NAME>` — Name of identity to lookup, default test identity used if not provided

###### **Options:**

* `--hd-path <HD_PATH>` — If identity is a seed phrase use this hd path, default is 0
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar keys fund`

Fund an identity on a test network

**Usage:** `stellar keys fund [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Name of identity to lookup, default test identity used if not provided

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--hd-path <HD_PATH>` — If identity is a seed phrase use this hd path, default is 0
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar keys generate`

Generate a new identity using a 24-word seed phrase The seed phrase can be stored in a config file (default) or in an OS-specific secure store

**Usage:** `stellar keys generate [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Name of identity

###### **Options:**

* `--seed <SEED>` — Optional seed to use when generating seed phrase. Random otherwise
* `-s`, `--as-secret` — Output the generated identity as a secret key
* `--secure-store` — Save the new key in your OS's credential secure store.

   On Mac this uses Keychain, on Windows it is Secure Store Service, and on *nix platforms it uses a combination of the kernel keyutils and DBus-based Secret Service.
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--hd-path <HD_PATH>` — When generating a secret key, which `hd_path` should be used from the original `seed_phrase`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--fund` — Fund generated key pair

  Default value: `false`
* `--overwrite` — Overwrite existing identity if it already exists



## `stellar keys ls`

List identities

**Usage:** `stellar keys ls [OPTIONS]`

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `-l`, `--long`



## `stellar keys rm`

Remove an identity

**Usage:** `stellar keys rm [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Identity to remove

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar keys secret`

Output an identity's secret key

**Usage:** `stellar keys secret [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Name of identity to lookup, default is test identity

###### **Options:**

* `--phrase` — Output seed phrase instead of private key
* `--hd-path <HD_PATH>` — If identity is a seed phrase use this hd path, default is 0
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar keys use`

Set the default identity that will be used on all commands. This allows you to skip `--source-account` or setting a environment variable, while reusing this value in all commands that require it

**Usage:** `stellar keys use [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Set the default network name

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar network`

Configure connection to networks

**Usage:** `stellar network <COMMAND>`

###### **Subcommands:**

* `add` — Add a new network
* `rm` — Remove a network
* `ls` — List networks
* `use` — Set the default network that will be used on all commands. This allows you to skip `--network` or setting a environment variable, while reusing this value in all commands that require it
* `health` — Fetch the health of the configured RPC
* `info` — Checks the health of the configured RPC
* `settings` — Fetch the network's config settings



## `stellar network add`

Add a new network

**Usage:** `stellar network add [OPTIONS] --rpc-url <RPC_URL> --network-passphrase <NETWORK_PASSPHRASE> <NAME>`

###### **Arguments:**

* `<NAME>` — Name of network

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — Optional header (e.g. API Key) to include in requests to the RPC
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar network rm`

Remove a network

**Usage:** `stellar network rm [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Network to remove

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar network ls`

List networks

**Usage:** `stellar network ls [OPTIONS]`

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `-l`, `--long` — Get more info about the networks



## `stellar network use`

Set the default network that will be used on all commands. This allows you to skip `--network` or setting a environment variable, while reusing this value in all commands that require it

**Usage:** `stellar network use [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Set the default network name

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar network health`

Fetch the health of the configured RPC

**Usage:** `stellar network health [OPTIONS]`

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--output <OUTPUT>` — Format of the output

  Default value: `text`

  Possible values:
  - `text`:
    Text output of network health status
  - `json`:
    JSON result of the RPC request
  - `json-formatted`:
    Formatted (multiline) JSON output of the RPC request




## `stellar network info`

Checks the health of the configured RPC

**Usage:** `stellar network info [OPTIONS]`

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--output <OUTPUT>` — Format of the output

  Default value: `text`

  Possible values:
  - `text`:
    Text output of network info
  - `json`:
    JSON result of the RPC request
  - `json-formatted`:
    Formatted (multiline) JSON output of the RPC request




## `stellar network settings`

Fetch the network's config settings

**Usage:** `stellar network settings [OPTIONS]`

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--internal` — Include internal config settings that are not upgradeable and are internally maintained by the network
* `--output <OUTPUT>` — Format of the output

  Default value: `json`

  Possible values:
  - `xdr`:
    XDR (`ConfigUpgradeSet` type)
  - `json`:
    JSON, XDR-JSON of the `ConfigUpgradeSet` XDR type
  - `json-formatted`:
    JSON formatted, XDR-JSON of the `ConfigUpgradeSet` XDR type




## `stellar container`

Start local networks in containers

**Usage:** `stellar container <COMMAND>`

###### **Subcommands:**

* `logs` — Get logs from a running network container
* `start` — Start a container running a Stellar node, RPC, API, and friendbot (faucet)
* `stop` — Stop a network container started with `stellar container start`



## `stellar container logs`

Get logs from a running network container

**Usage:** `stellar container logs [OPTIONS] [NAME]`

###### **Arguments:**

* `<NAME>` — Container to get logs from

  Default value: `local`

###### **Options:**

* `-d`, `--docker-host <DOCKER_HOST>` — Optional argument to override the default docker host. This is useful when you are using a non-standard docker host path for your Docker-compatible container runtime, e.g. Docker Desktop defaults to $HOME/.docker/run/docker.sock instead of /var/run/docker.sock



## `stellar container start`

Start a container running a Stellar node, RPC, API, and friendbot (faucet).

`stellar container start NETWORK [OPTIONS]`

By default, when starting a testnet container, without any optional arguments, it will run the equivalent of the following docker command:

`docker run --rm -p 8000:8000 --name stellar stellar/quickstart:testing --testnet --enable rpc,horizon`

**Usage:** `stellar container start [OPTIONS] [NETWORK]`

###### **Arguments:**

* `<NETWORK>` — Network to start. Default is `local`

  Possible values: `local`, `testnet`, `futurenet`, `pubnet`


###### **Options:**

* `-d`, `--docker-host <DOCKER_HOST>` — Optional argument to override the default docker host. This is useful when you are using a non-standard docker host path for your Docker-compatible container runtime, e.g. Docker Desktop defaults to $HOME/.docker/run/docker.sock instead of /var/run/docker.sock
* `--name <NAME>` — Optional argument to specify the container name
* `-l`, `--limits <LIMITS>` — Optional argument to specify the limits for the local network only
* `-p`, `--ports-mapping <PORTS_MAPPING>` — Argument to specify the `HOST_PORT:CONTAINER_PORT` mapping

  Default value: `8000:8000`
* `-t`, `--image-tag-override <IMAGE_TAG_OVERRIDE>` — Optional argument to override the default docker image tag for the given network
* `--protocol-version <PROTOCOL_VERSION>` — Optional argument to specify the protocol version for the local network only



## `stellar container stop`

Stop a network container started with `stellar container start`

**Usage:** `stellar container stop [OPTIONS] [NAME]`

###### **Arguments:**

* `<NAME>` — Container to stop

  Default value: `local`

###### **Options:**

* `-d`, `--docker-host <DOCKER_HOST>` — Optional argument to override the default docker host. This is useful when you are using a non-standard docker host path for your Docker-compatible container runtime, e.g. Docker Desktop defaults to $HOME/.docker/run/docker.sock instead of /var/run/docker.sock



## `stellar config`

Manage cli configuration

**Usage:** `stellar config <COMMAND>`

###### **Subcommands:**

* `migrate` — Migrate the local configuration to the global directory
* `dir` — Show the global configuration directory



## `stellar config migrate`

Migrate the local configuration to the global directory

**Usage:** `stellar config migrate [OPTIONS]`

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar config dir`

Show the global configuration directory.

The location will depend on how your system is configured.

- It looks up for `XDG_CONFIG_HOME` environment variable. If it's set, `$XDG_CONFIG_HOME/stellar` will be used. - If not set, it defaults to `$HOME/.config`. - Can be overridden by `--config-dir` flag.

**Usage:** `stellar config dir [OPTIONS]`

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar snapshot`

Download a snapshot of a ledger from an archive

**Usage:** `stellar snapshot <COMMAND>`

###### **Subcommands:**

* `create` — Create a ledger snapshot using a history archive



## `stellar snapshot create`

Create a ledger snapshot using a history archive.

Filters (address, wasm-hash) specify what ledger entries to include.

Account addresses include the account, and trustlines.

Contract addresses include the related wasm, contract data.

If a contract is a Stellar asset contract, it includes the asset issuer's account and trust lines, but does not include all the trust lines of other accounts holding the asset. To include them specify the addresses of relevant accounts.

Any invalid contract id passed as `--address` will be ignored.

**Usage:** `stellar snapshot create [OPTIONS] --output <OUTPUT>`

###### **Options:**

* `--ledger <LEDGER>` — The ledger sequence number to snapshot. Defaults to latest history archived ledger
* `--address <ADDRESS>` — Account or contract address/alias to include in the snapshot
* `--wasm-hash <WASM_HASHES>` — WASM hashes to include in the snapshot
* `--output <OUTPUT>` — Format of the out file

  Possible values: `json`

* `--out <OUT>` — Out path that the snapshot is written to

  Default value: `snapshot.json`
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--archive-url <ARCHIVE_URL>` — Archive URL



## `stellar tx`

Sign, Simulate, and Send transactions

**Usage:** `stellar tx <COMMAND>`

###### **Subcommands:**

* `update` — Update the transaction
* `edit` — Edit a transaction envelope from stdin. This command respects the environment variables `STELLAR_EDITOR`, `EDITOR` and `VISUAL`, in that order
* `hash` — Calculate the hash of a transaction envelope
* `new` — Create a new transaction
* `operation` — Manipulate the operations in a transaction, including adding new operations
* `send` — Send a transaction envelope to the network
* `sign` — Sign a transaction envelope appending the signature to the envelope
* `simulate` — Simulate a transaction envelope from stdin
* `fetch` — Fetch a transaction from the network by hash If no subcommand is passed in, the transaction envelope will be returned
* `decode` — Decode a transaction envelope from XDR to JSON
* `encode` — Encode a transaction envelope from JSON to XDR



## `stellar tx update`

Update the transaction

**Usage:** `stellar tx update <COMMAND>`

###### **Subcommands:**

* `sequence-number` — Edit the sequence number on a transaction



## `stellar tx update sequence-number`

Edit the sequence number on a transaction

**Usage:** `stellar tx update sequence-number <COMMAND>`

**Command Alias:** `seq-num`

###### **Subcommands:**

* `next` — Fetch the source account's seq-num and increment for the given tx



## `stellar tx update sequence-number next`

Fetch the source account's seq-num and increment for the given tx

**Usage:** `stellar tx update sequence-number next [OPTIONS]`

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar tx edit`

Edit a transaction envelope from stdin. This command respects the environment variables `STELLAR_EDITOR`, `EDITOR` and `VISUAL`, in that order.

Example: Start a new edit session

$ stellar tx edit

Example: Pipe an XDR transaction envelope

$ stellar tx new manage-data --data-name hello --build-only | stellar tx edit

**Usage:** `stellar tx edit`



## `stellar tx hash`

Calculate the hash of a transaction envelope

**Usage:** `stellar tx hash [OPTIONS] [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config



## `stellar tx new`

Create a new transaction

**Usage:** `stellar tx new <COMMAND>`

###### **Subcommands:**

* `account-merge` — Transfer XLM balance to another account and remove source account
* `bump-sequence` — Bump sequence number to invalidate older transactions
* `change-trust` — Create, update, or delete a trustline
* `claim-claimable-balance` — Claim a claimable balance by its balance ID
* `clawback` — Clawback an asset from an account
* `clawback-claimable-balance` — Clawback a claimable balance by its balance ID
* `create-account` — Create and fund a new account
* `create-claimable-balance` — Create a claimable balance that can be claimed by specified accounts
* `create-passive-sell-offer` — Create a passive sell offer on the Stellar DEX
* `manage-buy-offer` — Create, update, or delete a buy offer
* `manage-data` — Set, modify, or delete account data entries
* `manage-sell-offer` — Create, update, or delete a sell offer
* `path-payment-strict-send` — Send a payment with a different asset using path finding, specifying the send amount
* `path-payment-strict-receive` — Send a payment with a different asset using path finding, specifying the receive amount
* `payment` — Send asset to destination account
* `set-options` — Set account options like flags, signers, and home domain
* `set-trustline-flags` — Configure authorization and trustline flags for an asset



## `stellar tx new account-merge`

Transfer XLM balance to another account and remove source account

**Usage:** `stellar tx new account-merge [OPTIONS] --source-account <SOURCE_ACCOUNT> --account <ACCOUNT>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--account <ACCOUNT>` — Muxed Account to merge with, e.g. `GBX...`, 'MBX...'



## `stellar tx new bump-sequence`

Bump sequence number to invalidate older transactions

**Usage:** `stellar tx new bump-sequence [OPTIONS] --source-account <SOURCE_ACCOUNT> --bump-to <BUMP_TO>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--bump-to <BUMP_TO>` — Sequence number to bump to



## `stellar tx new change-trust`

Create, update, or delete a trustline

**Usage:** `stellar tx new change-trust [OPTIONS] --source-account <SOURCE_ACCOUNT> --line <LINE>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--line <LINE>`
* `--limit <LIMIT>` — Limit for the trust line, 0 to remove the trust line

  Default value: `9223372036854775807`



## `stellar tx new claim-claimable-balance`

Claim a claimable balance by its balance ID

**Usage:** `stellar tx new claim-claimable-balance [OPTIONS] --source-account <SOURCE_ACCOUNT> --balance-id <BALANCE_ID>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--balance-id <BALANCE_ID>` — Balance ID of the claimable balance to claim (64-character hex string)



## `stellar tx new clawback`

Clawback an asset from an account

**Usage:** `stellar tx new clawback [OPTIONS] --source-account <SOURCE_ACCOUNT> --from <FROM> --asset <ASSET> --amount <AMOUNT>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--from <FROM>` — Account to clawback assets from, e.g. `GBX...`
* `--asset <ASSET>` — Asset to clawback
* `--amount <AMOUNT>` — Amount of the asset to clawback, in stroops. 1 stroop = 0.0000001 of the asset



## `stellar tx new clawback-claimable-balance`

Clawback a claimable balance by its balance ID

**Usage:** `stellar tx new clawback-claimable-balance [OPTIONS] --source-account <SOURCE_ACCOUNT> --balance-id <BALANCE_ID>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--balance-id <BALANCE_ID>` — Balance ID of the claimable balance to clawback. Accepts multiple formats: - API format with type prefix (72 chars): 000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461 - Direct hash format (64 chars): 6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461 - StrKey format (base32): BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA



## `stellar tx new create-account`

Create and fund a new account

**Usage:** `stellar tx new create-account [OPTIONS] --source-account <SOURCE_ACCOUNT> --destination <DESTINATION>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--destination <DESTINATION>` — Account Id to create, e.g. `GBX...`
* `--starting-balance <STARTING_BALANCE>` — Initial balance in stroops of the account, default 1 XLM

  Default value: `10_000_000`



## `stellar tx new create-claimable-balance`

Create a claimable balance that can be claimed by specified accounts

**Usage:** `stellar tx new create-claimable-balance [OPTIONS] --source-account <SOURCE_ACCOUNT> --amount <AMOUNT>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--asset <ASSET>` — Asset to be held in the ClaimableBalanceEntry

  Default value: `native`
* `--amount <AMOUNT>` — Amount of asset to store in the entry, in stroops. 1 stroop = 0.0000001 of the asset
* `--claimant <CLAIMANTS>` — Claimants of the claimable balance. Format: account_id or account_id:predicate_json Can be specified multiple times for multiple claimants.

   Examples:

   - `--claimant alice (unconditional)` - `--claimant 'bob:{"before_absolute_time":"1735689599"}'` - `--claimant 'charlie:{"and":[{"before_absolute_time":"1735689599"},{"before_relative_time":"3600"}]}'`



## `stellar tx new create-passive-sell-offer`

Create a passive sell offer on the Stellar DEX

**Usage:** `stellar tx new create-passive-sell-offer [OPTIONS] --source-account <SOURCE_ACCOUNT> --selling <SELLING> --buying <BUYING> --amount <AMOUNT> --price <PRICE>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--selling <SELLING>` — Asset to sell
* `--buying <BUYING>` — Asset to buy
* `--amount <AMOUNT>` — Amount of selling asset to offer, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops)
* `--price <PRICE>` — Price of 1 unit of selling asset in terms of buying asset as "numerator:denominator" (e.g., "1:2" means 0.5)



## `stellar tx new manage-buy-offer`

Create, update, or delete a buy offer

**Usage:** `stellar tx new manage-buy-offer [OPTIONS] --source-account <SOURCE_ACCOUNT> --selling <SELLING> --buying <BUYING> --amount <AMOUNT> --price <PRICE>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--selling <SELLING>` — Asset to sell
* `--buying <BUYING>` — Asset to buy
* `--amount <AMOUNT>` — Amount of buying asset to purchase, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops). Use `0` to remove the offer
* `--price <PRICE>` — Price of 1 unit of buying asset in terms of selling asset as "numerator:denominator" (e.g., "1:2" means 0.5)
* `--offer-id <OFFER_ID>` — Offer ID. If 0, will create new offer. Otherwise, will update existing offer

  Default value: `0`



## `stellar tx new manage-data`

Set, modify, or delete account data entries

**Usage:** `stellar tx new manage-data [OPTIONS] --source-account <SOURCE_ACCOUNT> --data-name <DATA_NAME>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--data-name <DATA_NAME>` — String up to 64 bytes long. If this is a new Name it will add the given name/value pair to the account. If this Name is already present then the associated value will be modified
* `--data-value <DATA_VALUE>` — Up to 64 bytes long hex string If not present then the existing Name will be deleted. If present then this value will be set in the `DataEntry`



## `stellar tx new manage-sell-offer`

Create, update, or delete a sell offer

**Usage:** `stellar tx new manage-sell-offer [OPTIONS] --source-account <SOURCE_ACCOUNT> --selling <SELLING> --buying <BUYING> --amount <AMOUNT> --price <PRICE>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--selling <SELLING>` — Asset to sell
* `--buying <BUYING>` — Asset to buy
* `--amount <AMOUNT>` — Amount of selling asset to offer, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops). Use `0` to remove the offer
* `--price <PRICE>` — Price of 1 unit of selling asset in terms of buying asset as "numerator:denominator" (e.g., "1:2" means 0.5)
* `--offer-id <OFFER_ID>` — Offer ID. If 0, will create new offer. Otherwise, will update existing offer

  Default value: `0`



## `stellar tx new path-payment-strict-send`

Send a payment with a different asset using path finding, specifying the send amount

**Usage:** `stellar tx new path-payment-strict-send [OPTIONS] --source-account <SOURCE_ACCOUNT> --send-asset <SEND_ASSET> --send-amount <SEND_AMOUNT> --destination <DESTINATION> --dest-asset <DEST_ASSET> --dest-min <DEST_MIN>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--send-asset <SEND_ASSET>` — Asset to send (pay with)
* `--send-amount <SEND_AMOUNT>` — Amount of send asset to deduct from sender's account, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops)
* `--destination <DESTINATION>` — Account that receives the payment
* `--dest-asset <DEST_ASSET>` — Asset that the destination will receive
* `--dest-min <DEST_MIN>` — Minimum amount of destination asset that the destination account can receive. The operation will fail if this amount cannot be met
* `--path <PATH>` — List of intermediate assets for the payment path, comma-separated (up to 5 assets). Each asset should be in the format 'code:issuer' or 'native' for XLM



## `stellar tx new path-payment-strict-receive`

Send a payment with a different asset using path finding, specifying the receive amount

**Usage:** `stellar tx new path-payment-strict-receive [OPTIONS] --source-account <SOURCE_ACCOUNT> --send-asset <SEND_ASSET> --send-max <SEND_MAX> --destination <DESTINATION> --dest-asset <DEST_ASSET> --dest-amount <DEST_AMOUNT>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--send-asset <SEND_ASSET>` — Asset to send (pay with)
* `--send-max <SEND_MAX>` — Maximum amount of send asset to deduct from sender's account, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops)
* `--destination <DESTINATION>` — Account that receives the payment
* `--dest-asset <DEST_ASSET>` — Asset that the destination will receive
* `--dest-amount <DEST_AMOUNT>` — Exact amount of destination asset that the destination account will receive, in stroops. 1 stroop = 0.0000001 of the asset
* `--path <PATH>` — List of intermediate assets for the payment path, comma-separated (up to 5 assets). Each asset should be in the format 'code:issuer' or 'native' for XLM



## `stellar tx new payment`

Send asset to destination account

**Usage:** `stellar tx new payment [OPTIONS] --source-account <SOURCE_ACCOUNT> --destination <DESTINATION> --amount <AMOUNT>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--destination <DESTINATION>` — Account to send to, e.g. `GBX...`
* `--asset <ASSET>` — Asset to send, default native, e.i. XLM

  Default value: `native`
* `--amount <AMOUNT>` — Amount of the aforementioned asset to send, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops)



## `stellar tx new set-options`

Set account options like flags, signers, and home domain

**Usage:** `stellar tx new set-options [OPTIONS] --source-account <SOURCE_ACCOUNT>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--inflation-dest <INFLATION_DEST>` — Account of the inflation destination
* `--master-weight <MASTER_WEIGHT>` — A number from 0-255 (inclusive) representing the weight of the master key. If the weight of the master key is updated to 0, it is effectively disabled
* `--low-threshold <LOW_THRESHOLD>` — A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have a low threshold. https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
* `--med-threshold <MED_THRESHOLD>` — A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have a medium threshold. https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
* `--high-threshold <HIGH_THRESHOLD>` — A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have a high threshold. https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
* `--home-domain <HOME_DOMAIN>` — Sets the home domain of an account. See https://developers.stellar.org/docs/learn/encyclopedia/network-configuration/federation
* `--signer <SIGNER>` — Add, update, or remove a signer from an account
* `--signer-weight <SIGNER_WEIGHT>` — Signer weight is a number from 0-255 (inclusive). The signer is deleted if the weight is 0
* `--set-required` — When enabled, an issuer must approve an account before that account can hold its asset. https://developers.stellar.org/docs/tokens/control-asset-access#authorization-required-0x1
* `--set-revocable` — When enabled, an issuer can revoke an existing trustline's authorization, thereby freezing the asset held by an account. https://developers.stellar.org/docs/tokens/control-asset-access#authorization-revocable-0x2
* `--set-clawback-enabled` — Enables the issuing account to take back (burning) all of the asset. https://developers.stellar.org/docs/tokens/control-asset-access#clawback-enabled-0x8
* `--set-immutable` — With this setting, none of the other authorization flags (`AUTH_REQUIRED_FLAG`, `AUTH_REVOCABLE_FLAG`) can be set, and the issuing account can't be merged. https://developers.stellar.org/docs/tokens/control-asset-access#authorization-immutable-0x4
* `--clear-required`
* `--clear-revocable`
* `--clear-immutable`
* `--clear-clawback-enabled`



## `stellar tx new set-trustline-flags`

Configure authorization and trustline flags for an asset

**Usage:** `stellar tx new set-trustline-flags [OPTIONS] --source-account <SOURCE_ACCOUNT> --trustor <TRUSTOR> --asset <ASSET>`

###### **Options:**

* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--trustor <TRUSTOR>` — Account to set trustline flags for, e.g. `GBX...`, or alias, or muxed account, `M123...``
* `--asset <ASSET>` — Asset to set trustline flags for
* `--set-authorize` — Signifies complete authorization allowing an account to transact freely with the asset to make and receive payments and place orders
* `--set-authorize-to-maintain-liabilities` — Denotes limited authorization that allows an account to maintain current orders but not to otherwise transact with the asset
* `--set-trustline-clawback-enabled` — Enables the issuing account to take back (burning) all of the asset. See our section on Clawbacks: https://developers.stellar.org/docs/learn/encyclopedia/transactions-specialized/clawbacks
* `--clear-authorize`
* `--clear-authorize-to-maintain-liabilities`
* `--clear-trustline-clawback-enabled`



## `stellar tx operation`

Manipulate the operations in a transaction, including adding new operations

**Usage:** `stellar tx operation <COMMAND>`

**Command Alias:** `op`

###### **Subcommands:**

* `add` — Add Operation to a transaction



## `stellar tx operation add`

Add Operation to a transaction

**Usage:** `stellar tx operation add <COMMAND>`

###### **Subcommands:**

* `account-merge` — Transfer XLM balance to another account and remove source account
* `bump-sequence` — Bump sequence number to invalidate older transactions
* `change-trust` — Create, update, or delete a trustline
* `claim-claimable-balance` — Claim a claimable balance by its balance ID
* `clawback` — Clawback an asset from an account
* `clawback-claimable-balance` — Clawback a claimable balance by its balance ID
* `create-account` — Create and fund a new account
* `create-claimable-balance` — Create a claimable balance that can be claimed by specified accounts
* `create-passive-sell-offer` — Create a passive sell offer on the Stellar DEX
* `manage-buy-offer` — Create, update, or delete a buy offer
* `manage-data` — Set, modify, or delete account data entries
* `manage-sell-offer` — Create, update, or delete a sell offer
* `path-payment-strict-receive` — Send a payment with a different asset using path finding, specifying the receive amount
* `path-payment-strict-send` — Send a payment with a different asset using path finding, specifying the send amount
* `payment` — Send asset to destination account
* `set-options` — Set account options like flags, signers, and home domain
* `set-trustline-flags` — Configure authorization and trustline flags for an asset



## `stellar tx operation add account-merge`

Transfer XLM balance to another account and remove source account

**Usage:** `stellar tx operation add account-merge [OPTIONS] --source-account <SOURCE_ACCOUNT> --account <ACCOUNT> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--account <ACCOUNT>` — Muxed Account to merge with, e.g. `GBX...`, 'MBX...'



## `stellar tx operation add bump-sequence`

Bump sequence number to invalidate older transactions

**Usage:** `stellar tx operation add bump-sequence [OPTIONS] --source-account <SOURCE_ACCOUNT> --bump-to <BUMP_TO> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--bump-to <BUMP_TO>` — Sequence number to bump to



## `stellar tx operation add change-trust`

Create, update, or delete a trustline

**Usage:** `stellar tx operation add change-trust [OPTIONS] --source-account <SOURCE_ACCOUNT> --line <LINE> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--line <LINE>`
* `--limit <LIMIT>` — Limit for the trust line, 0 to remove the trust line

  Default value: `9223372036854775807`



## `stellar tx operation add claim-claimable-balance`

Claim a claimable balance by its balance ID

**Usage:** `stellar tx operation add claim-claimable-balance [OPTIONS] --source-account <SOURCE_ACCOUNT> --balance-id <BALANCE_ID> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--balance-id <BALANCE_ID>` — Balance ID of the claimable balance to claim (64-character hex string)



## `stellar tx operation add clawback`

Clawback an asset from an account

**Usage:** `stellar tx operation add clawback [OPTIONS] --source-account <SOURCE_ACCOUNT> --from <FROM> --asset <ASSET> --amount <AMOUNT> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--from <FROM>` — Account to clawback assets from, e.g. `GBX...`
* `--asset <ASSET>` — Asset to clawback
* `--amount <AMOUNT>` — Amount of the asset to clawback, in stroops. 1 stroop = 0.0000001 of the asset



## `stellar tx operation add clawback-claimable-balance`

Clawback a claimable balance by its balance ID

**Usage:** `stellar tx operation add clawback-claimable-balance [OPTIONS] --source-account <SOURCE_ACCOUNT> --balance-id <BALANCE_ID> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--balance-id <BALANCE_ID>` — Balance ID of the claimable balance to clawback. Accepts multiple formats: - API format with type prefix (72 chars): 000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461 - Direct hash format (64 chars): 6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461 - StrKey format (base32): BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA



## `stellar tx operation add create-account`

Create and fund a new account

**Usage:** `stellar tx operation add create-account [OPTIONS] --source-account <SOURCE_ACCOUNT> --destination <DESTINATION> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--destination <DESTINATION>` — Account Id to create, e.g. `GBX...`
* `--starting-balance <STARTING_BALANCE>` — Initial balance in stroops of the account, default 1 XLM

  Default value: `10_000_000`



## `stellar tx operation add create-claimable-balance`

Create a claimable balance that can be claimed by specified accounts

**Usage:** `stellar tx operation add create-claimable-balance [OPTIONS] --source-account <SOURCE_ACCOUNT> --amount <AMOUNT> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--asset <ASSET>` — Asset to be held in the ClaimableBalanceEntry

  Default value: `native`
* `--amount <AMOUNT>` — Amount of asset to store in the entry, in stroops. 1 stroop = 0.0000001 of the asset
* `--claimant <CLAIMANTS>` — Claimants of the claimable balance. Format: account_id or account_id:predicate_json Can be specified multiple times for multiple claimants.

   Examples:

   - `--claimant alice (unconditional)` - `--claimant 'bob:{"before_absolute_time":"1735689599"}'` - `--claimant 'charlie:{"and":[{"before_absolute_time":"1735689599"},{"before_relative_time":"3600"}]}'`



## `stellar tx operation add create-passive-sell-offer`

Create a passive sell offer on the Stellar DEX

**Usage:** `stellar tx operation add create-passive-sell-offer [OPTIONS] --source-account <SOURCE_ACCOUNT> --selling <SELLING> --buying <BUYING> --amount <AMOUNT> --price <PRICE> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--selling <SELLING>` — Asset to sell
* `--buying <BUYING>` — Asset to buy
* `--amount <AMOUNT>` — Amount of selling asset to offer, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops)
* `--price <PRICE>` — Price of 1 unit of selling asset in terms of buying asset as "numerator:denominator" (e.g., "1:2" means 0.5)



## `stellar tx operation add manage-buy-offer`

Create, update, or delete a buy offer

**Usage:** `stellar tx operation add manage-buy-offer [OPTIONS] --source-account <SOURCE_ACCOUNT> --selling <SELLING> --buying <BUYING> --amount <AMOUNT> --price <PRICE> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--selling <SELLING>` — Asset to sell
* `--buying <BUYING>` — Asset to buy
* `--amount <AMOUNT>` — Amount of buying asset to purchase, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops). Use `0` to remove the offer
* `--price <PRICE>` — Price of 1 unit of buying asset in terms of selling asset as "numerator:denominator" (e.g., "1:2" means 0.5)
* `--offer-id <OFFER_ID>` — Offer ID. If 0, will create new offer. Otherwise, will update existing offer

  Default value: `0`



## `stellar tx operation add manage-data`

Set, modify, or delete account data entries

**Usage:** `stellar tx operation add manage-data [OPTIONS] --source-account <SOURCE_ACCOUNT> --data-name <DATA_NAME> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--data-name <DATA_NAME>` — String up to 64 bytes long. If this is a new Name it will add the given name/value pair to the account. If this Name is already present then the associated value will be modified
* `--data-value <DATA_VALUE>` — Up to 64 bytes long hex string If not present then the existing Name will be deleted. If present then this value will be set in the `DataEntry`



## `stellar tx operation add manage-sell-offer`

Create, update, or delete a sell offer

**Usage:** `stellar tx operation add manage-sell-offer [OPTIONS] --source-account <SOURCE_ACCOUNT> --selling <SELLING> --buying <BUYING> --amount <AMOUNT> --price <PRICE> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--selling <SELLING>` — Asset to sell
* `--buying <BUYING>` — Asset to buy
* `--amount <AMOUNT>` — Amount of selling asset to offer, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops). Use `0` to remove the offer
* `--price <PRICE>` — Price of 1 unit of selling asset in terms of buying asset as "numerator:denominator" (e.g., "1:2" means 0.5)
* `--offer-id <OFFER_ID>` — Offer ID. If 0, will create new offer. Otherwise, will update existing offer

  Default value: `0`



## `stellar tx operation add path-payment-strict-receive`

Send a payment with a different asset using path finding, specifying the receive amount

**Usage:** `stellar tx operation add path-payment-strict-receive [OPTIONS] --source-account <SOURCE_ACCOUNT> --send-asset <SEND_ASSET> --send-max <SEND_MAX> --destination <DESTINATION> --dest-asset <DEST_ASSET> --dest-amount <DEST_AMOUNT> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--send-asset <SEND_ASSET>` — Asset to send (pay with)
* `--send-max <SEND_MAX>` — Maximum amount of send asset to deduct from sender's account, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops)
* `--destination <DESTINATION>` — Account that receives the payment
* `--dest-asset <DEST_ASSET>` — Asset that the destination will receive
* `--dest-amount <DEST_AMOUNT>` — Exact amount of destination asset that the destination account will receive, in stroops. 1 stroop = 0.0000001 of the asset
* `--path <PATH>` — List of intermediate assets for the payment path, comma-separated (up to 5 assets). Each asset should be in the format 'code:issuer' or 'native' for XLM



## `stellar tx operation add path-payment-strict-send`

Send a payment with a different asset using path finding, specifying the send amount

**Usage:** `stellar tx operation add path-payment-strict-send [OPTIONS] --source-account <SOURCE_ACCOUNT> --send-asset <SEND_ASSET> --send-amount <SEND_AMOUNT> --destination <DESTINATION> --dest-asset <DEST_ASSET> --dest-min <DEST_MIN> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--send-asset <SEND_ASSET>` — Asset to send (pay with)
* `--send-amount <SEND_AMOUNT>` — Amount of send asset to deduct from sender's account, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops)
* `--destination <DESTINATION>` — Account that receives the payment
* `--dest-asset <DEST_ASSET>` — Asset that the destination will receive
* `--dest-min <DEST_MIN>` — Minimum amount of destination asset that the destination account can receive. The operation will fail if this amount cannot be met
* `--path <PATH>` — List of intermediate assets for the payment path, comma-separated (up to 5 assets). Each asset should be in the format 'code:issuer' or 'native' for XLM



## `stellar tx operation add payment`

Send asset to destination account

**Usage:** `stellar tx operation add payment [OPTIONS] --source-account <SOURCE_ACCOUNT> --destination <DESTINATION> --amount <AMOUNT> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--destination <DESTINATION>` — Account to send to, e.g. `GBX...`
* `--asset <ASSET>` — Asset to send, default native, e.i. XLM

  Default value: `native`
* `--amount <AMOUNT>` — Amount of the aforementioned asset to send, in stroops. 1 stroop = 0.0000001 of the asset (e.g. 1 XLM = `10_000_000` stroops)



## `stellar tx operation add set-options`

Set account options like flags, signers, and home domain

**Usage:** `stellar tx operation add set-options [OPTIONS] --source-account <SOURCE_ACCOUNT> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--inflation-dest <INFLATION_DEST>` — Account of the inflation destination
* `--master-weight <MASTER_WEIGHT>` — A number from 0-255 (inclusive) representing the weight of the master key. If the weight of the master key is updated to 0, it is effectively disabled
* `--low-threshold <LOW_THRESHOLD>` — A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have a low threshold. https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
* `--med-threshold <MED_THRESHOLD>` — A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have a medium threshold. https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
* `--high-threshold <HIGH_THRESHOLD>` — A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have a high threshold. https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
* `--home-domain <HOME_DOMAIN>` — Sets the home domain of an account. See https://developers.stellar.org/docs/learn/encyclopedia/network-configuration/federation
* `--signer <SIGNER>` — Add, update, or remove a signer from an account
* `--signer-weight <SIGNER_WEIGHT>` — Signer weight is a number from 0-255 (inclusive). The signer is deleted if the weight is 0
* `--set-required` — When enabled, an issuer must approve an account before that account can hold its asset. https://developers.stellar.org/docs/tokens/control-asset-access#authorization-required-0x1
* `--set-revocable` — When enabled, an issuer can revoke an existing trustline's authorization, thereby freezing the asset held by an account. https://developers.stellar.org/docs/tokens/control-asset-access#authorization-revocable-0x2
* `--set-clawback-enabled` — Enables the issuing account to take back (burning) all of the asset. https://developers.stellar.org/docs/tokens/control-asset-access#clawback-enabled-0x8
* `--set-immutable` — With this setting, none of the other authorization flags (`AUTH_REQUIRED_FLAG`, `AUTH_REVOCABLE_FLAG`) can be set, and the issuing account can't be merged. https://developers.stellar.org/docs/tokens/control-asset-access#authorization-immutable-0x4
* `--clear-required`
* `--clear-revocable`
* `--clear-immutable`
* `--clear-clawback-enabled`



## `stellar tx operation add set-trustline-flags`

Configure authorization and trustline flags for an asset

**Usage:** `stellar tx operation add set-trustline-flags [OPTIONS] --source-account <SOURCE_ACCOUNT> --trustor <TRUSTOR> --asset <ASSET> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--operation-source-account <OPERATION_SOURCE_ACCOUNT>` [alias: `op-source`] — Source account used for the operation
* `--fee <FEE>` — fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm

  Default value: `100`
* `--cost` — Output the cost execution to stderr
* `--instructions <INSTRUCTIONS>` — Number of instructions to simulate
* `--build-only` — Build the transaction and only write the base64 xdr to stdout
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--trustor <TRUSTOR>` — Account to set trustline flags for, e.g. `GBX...`, or alias, or muxed account, `M123...``
* `--asset <ASSET>` — Asset to set trustline flags for
* `--set-authorize` — Signifies complete authorization allowing an account to transact freely with the asset to make and receive payments and place orders
* `--set-authorize-to-maintain-liabilities` — Denotes limited authorization that allows an account to maintain current orders but not to otherwise transact with the asset
* `--set-trustline-clawback-enabled` — Enables the issuing account to take back (burning) all of the asset. See our section on Clawbacks: https://developers.stellar.org/docs/learn/encyclopedia/transactions-specialized/clawbacks
* `--clear-authorize`
* `--clear-authorize-to-maintain-liabilities`
* `--clear-trustline-clawback-enabled`



## `stellar tx send`

Send a transaction envelope to the network

**Usage:** `stellar tx send [OPTIONS] [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar tx sign`

Sign a transaction envelope appending the signature to the envelope

**Usage:** `stellar tx sign [OPTIONS] [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR, or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings



## `stellar tx simulate`

Simulate a transaction envelope from stdin

**Usage:** `stellar tx simulate [OPTIONS] --source-account <SOURCE_ACCOUNT> [TX_XDR]`

###### **Arguments:**

* `<TX_XDR>` — Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `-s`, `--source-account <SOURCE_ACCOUNT>` [alias: `source`] — Account that where transaction originates from. Alias `source`. Can be an identity (--source alice), a public key (--source GDKW...), a muxed account (--source MDA…), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to sign the final transaction. In that case, trying to sign with public key will fail
* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `--sign-with-key <SIGN_WITH_KEY>` — Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path
* `--hd-path <HD_PATH>` — If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--sign-with-lab` — Sign with https://lab.stellar.org
* `--sign-with-ledger` — Sign with a ledger wallet



## `stellar tx fetch`

Fetch a transaction from the network by hash If no subcommand is passed in, the transaction envelope will be returned

**Usage:** `stellar tx fetch [OPTIONS]
       fetch <COMMAND>`

###### **Subcommands:**

* `result` — Fetch the transaction result
* `meta` — Fetch the transaction meta
* `fee` — Fetch the transaction fee information

###### **Options:**

* `--hash <HASH>` — Hash of transaction to fetch
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--output <OUTPUT>` — Format of the output

  Default value: `json`

  Possible values:
  - `json`:
    JSON output of the ledger entry with parsed XDRs (one line, not formatted)
  - `json-formatted`:
    Formatted (multiline) JSON output of the ledger entry with parsed XDRs
  - `xdr`:
    Original RPC output (containing XDRs)




## `stellar tx fetch result`

Fetch the transaction result

**Usage:** `stellar tx fetch result [OPTIONS] --hash <HASH>`

###### **Options:**

* `--hash <HASH>` — Transaction hash to fetch
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--output <OUTPUT>` — Format of the output

  Default value: `json`

  Possible values:
  - `json`:
    JSON output of the ledger entry with parsed XDRs (one line, not formatted)
  - `json-formatted`:
    Formatted (multiline) JSON output of the ledger entry with parsed XDRs
  - `xdr`:
    Original RPC output (containing XDRs)




## `stellar tx fetch meta`

Fetch the transaction meta

**Usage:** `stellar tx fetch meta [OPTIONS] --hash <HASH>`

###### **Options:**

* `--hash <HASH>` — Transaction hash to fetch
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--output <OUTPUT>` — Format of the output

  Default value: `json`

  Possible values:
  - `json`:
    JSON output of the ledger entry with parsed XDRs (one line, not formatted)
  - `json-formatted`:
    Formatted (multiline) JSON output of the ledger entry with parsed XDRs
  - `xdr`:
    Original RPC output (containing XDRs)




## `stellar tx fetch fee`

Fetch the transaction fee information

**Usage:** `stellar tx fetch fee [OPTIONS] --hash <HASH>`

###### **Options:**

* `--hash <HASH>` — Transaction hash to fetch
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--output <OUTPUT>` — Output format for fee command

  Default value: `table`

  Possible values:
  - `json`:
    JSON output of the ledger entry with parsed XDRs (one line, not formatted)
  - `json-formatted`:
    Formatted (multiline) JSON output of the ledger entry with parsed XDRs
  - `table`:
    Formatted in a table comparing fee types




## `stellar tx decode`

Decode a transaction envelope from XDR to JSON

**Usage:** `stellar tx decode [OPTIONS] [INPUT]...`

###### **Arguments:**

* `<INPUT>` — XDR or files containing XDR to decode, or stdin if empty

###### **Options:**

* `--input <INPUT_FORMAT>`

  Default value: `single-base64`

  Possible values: `single-base64`, `single`

* `--output <OUTPUT_FORMAT>`

  Default value: `json`

  Possible values: `json`, `json-formatted`




## `stellar tx encode`

Encode a transaction envelope from JSON to XDR

**Usage:** `stellar tx encode [OPTIONS] [INPUT]...`

###### **Arguments:**

* `<INPUT>` — XDR or files containing XDR to decode, or stdin if empty

###### **Options:**

* `--input <INPUT_FORMAT>`

  Default value: `json`

  Possible values: `json`

* `--output <OUTPUT_FORMAT>`

  Default value: `single-base64`

  Possible values: `single-base64`, `single`




## `stellar xdr`

Decode and encode XDR

**Usage:** `stellar xdr [CHANNEL] <COMMAND>`

###### **Subcommands:**

* `types` — View information about types
* `guess` — Guess the XDR type
* `decode` — Decode XDR
* `encode` — Encode XDR
* `compare` — Compare two XDR values with each other
* `generate` — Generate XDR values
* `version` — Print version information

###### **Arguments:**

* `<CHANNEL>` — Channel of XDR to operate on

  Default value: `+curr`

  Possible values: `+curr`, `+next`




## `stellar xdr types`

View information about types

**Usage:** `stellar xdr types <COMMAND>`

###### **Subcommands:**

* `list` — 
* `schema` — 
* `schema-files` — Generate JSON schema files for the XDR types, writing a file for each type to the out directory



## `stellar xdr types list`

**Usage:** `stellar xdr types list [OPTIONS]`

###### **Options:**

* `--output <OUTPUT>`

  Default value: `plain`

  Possible values: `plain`, `json`, `json-formatted`




## `stellar xdr types schema`

**Usage:** `stellar xdr types schema [OPTIONS] --type <TYPE>`

###### **Options:**

* `--type <TYPE>` — XDR type to decode
* `--output <OUTPUT>`

  Default value: `json-schema-draft201909`

  Possible values: `json-schema-draft201909`




## `stellar xdr types schema-files`

Generate JSON schema files for the XDR types, writing a file for each type to the out directory

**Usage:** `stellar xdr types schema-files [OPTIONS] --out-dir <OUT_DIR>`

###### **Options:**

* `--out-dir <OUT_DIR>`
* `--output <OUTPUT>`

  Default value: `json-schema-draft201909`

  Possible values: `json-schema-draft201909`




## `stellar xdr guess`

Guess the XDR type.

Prints a list of types that the XDR values can be decoded into.

**Usage:** `stellar xdr guess [OPTIONS] [INPUT]`

###### **Arguments:**

* `<INPUT>` — XDR or file containing XDR to decode, or stdin if empty

###### **Options:**

* `--input <INPUT_FORMAT>`

  Default value: `single-base64`

  Possible values: `single`, `single-base64`, `stream`, `stream-base64`, `stream-framed`

* `--output <OUTPUT_FORMAT>`

  Default value: `list`

  Possible values: `list`

* `--certainty <CERTAINTY>` — Certainty as an arbitrary value

  Default value: `2`



## `stellar xdr decode`

Decode XDR

**Usage:** `stellar xdr decode [OPTIONS] --type <TYPE> [INPUT]...`

###### **Arguments:**

* `<INPUT>` — XDR or files containing XDR to decode, or stdin if empty

###### **Options:**

* `--type <TYPE>` — XDR type to decode
* `--input <INPUT_FORMAT>`

  Default value: `stream-base64`

  Possible values: `single`, `single-base64`, `stream`, `stream-base64`, `stream-framed`

* `--output <OUTPUT_FORMAT>`

  Default value: `json`

  Possible values: `json`, `json-formatted`, `text`, `rust-debug`, `rust-debug-formatted`




## `stellar xdr encode`

Encode XDR

**Usage:** `stellar xdr encode [OPTIONS] --type <TYPE> [INPUT]...`

###### **Arguments:**

* `<INPUT>` — XDR or files containing XDR to decode, or stdin if empty

###### **Options:**

* `--type <TYPE>` — XDR type to encode
* `--input <INPUT_FORMAT>`

  Default value: `json`

  Possible values: `json`

* `--output <OUTPUT_FORMAT>`

  Default value: `single-base64`

  Possible values: `single`, `single-base64`, `stream`




## `stellar xdr compare`

Compare two XDR values with each other

Outputs: `-1` when the left XDR value is less than the right XDR value, `0` when the left XDR value is equal to the right XDR value, `1` when the left XDR value is greater than the right XDR value

**Usage:** `stellar xdr compare [OPTIONS] --type <TYPE> <LEFT> <RIGHT>`

###### **Arguments:**

* `<LEFT>` — XDR file to decode and compare with the right value
* `<RIGHT>` — XDR file to decode and compare with the left value

###### **Options:**

* `--type <TYPE>` — XDR type of both inputs
* `--input <INPUT>`

  Default value: `single-base64`

  Possible values: `single`, `single-base64`




## `stellar xdr generate`

Generate XDR values

**Usage:** `stellar xdr generate <COMMAND>`

###### **Subcommands:**

* `default` — Generate default XDR values
* `arbitrary` — Generate arbitrary XDR values



## `stellar xdr generate default`

Generate default XDR values

**Usage:** `stellar xdr generate default [OPTIONS] --type <TYPE>`

###### **Options:**

* `--type <TYPE>` — XDR type to generate
* `--output <OUTPUT_FORMAT>`

  Default value: `single-base64`

  Possible values: `single`, `single-base64`, `json`, `json-formatted`, `text`




## `stellar xdr generate arbitrary`

Generate arbitrary XDR values

**Usage:** `stellar xdr generate arbitrary [OPTIONS] --type <TYPE>`

###### **Options:**

* `--type <TYPE>` — XDR type to generate
* `--output <OUTPUT_FORMAT>`

  Default value: `single-base64`

  Possible values: `single`, `single-base64`, `json`, `json-formatted`, `text`




## `stellar xdr version`

Print version information

**Usage:** `stellar xdr version`



## `stellar completion`

Print shell completion code for the specified shell

Ensure the completion package for your shell is installed, e.g. bash-completion for bash.

To enable autocomplete in the current bash shell, run: `source <(stellar completion --shell bash)`

To enable autocomplete permanently, run: `echo "source <(stellar completion --shell bash)" >> ~/.bashrc`


**Usage:** `stellar completion --shell <SHELL>`

###### **Options:**

* `--shell <SHELL>` — The shell type

  Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`




## `stellar cache`

Cache for transactions and contract specs

**Usage:** `stellar cache <COMMAND>`

###### **Subcommands:**

* `clean` — Delete the cache
* `path` — Show the location of the cache
* `actionlog` — Access details about cached actions like transactions, and simulations. (Experimental. May see breaking changes at any time.)



## `stellar cache clean`

Delete the cache

**Usage:** `stellar cache clean`



## `stellar cache path`

Show the location of the cache

**Usage:** `stellar cache path`



## `stellar cache actionlog`

Access details about cached actions like transactions, and simulations. (Experimental. May see breaking changes at any time.)

**Usage:** `stellar cache actionlog <COMMAND>`

###### **Subcommands:**

* `ls` — List cached actions (transactions, simulations)
* `read` — Read cached action



## `stellar cache actionlog ls`

List cached actions (transactions, simulations)

**Usage:** `stellar cache actionlog ls [OPTIONS]`

###### **Options:**

* `--global` — ⚠️ Deprecated: global config is always on
* `--config-dir <CONFIG_DIR>` — Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise. Contains configuration files, aliases, and other persistent settings
* `-l`, `--long`



## `stellar cache actionlog read`

Read cached action

**Usage:** `stellar cache actionlog read --id <ID>`

###### **Options:**

* `--id <ID>` — ID of the cache entry



## `stellar version`

Print version information

**Usage:** `stellar version [OPTIONS]`

###### **Options:**

* `--only-version` — Print only the version
* `--only-version-major` — Print only the major version



## `stellar plugin`

The subcommand for CLI plugins

**Usage:** `stellar plugin <COMMAND>`

###### **Subcommands:**

* `search` — Search for CLI plugins using GitHub
* `ls` — List installed plugins



## `stellar plugin search`

Search for CLI plugins using GitHub

**Usage:** `stellar plugin search`



## `stellar plugin ls`

List installed plugins

**Usage:** `stellar plugin ls`



## `stellar ledger`

Fetch ledger information

**Usage:** `stellar ledger <COMMAND>`

###### **Subcommands:**

* `latest` — Get the latest ledger sequence and information from the network
* `fetch` — 



## `stellar ledger latest`

Get the latest ledger sequence and information from the network

**Usage:** `stellar ledger latest [OPTIONS]`

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--output <OUTPUT>` — Format of the output

  Default value: `text`

  Possible values:
  - `text`:
    Text output of network info
  - `json`:
    JSON result of the RPC request
  - `json-formatted`:
    Formatted (multiline) JSON output of the RPC request




## `stellar ledger fetch`

**Usage:** `stellar ledger fetch [OPTIONS] <SEQ>`

###### **Arguments:**

* `<SEQ>` — Ledger Sequence to start fetch (inclusive)

###### **Options:**

* `--limit <LIMIT>` — Number of ledgers to fetch

  Default value: `1`
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--output <OUTPUT>` — Format of the output

  Default value: `text`

  Possible values:
  - `text`:
    Text output of network info
  - `json`:
    JSON result of the RPC request
  - `json-formatted`:
    Formatted (multiline) JSON output of the RPC request

* `--xdr-format <XDR_FORMAT>` — Format of the xdr in the output

  Default value: `json`

  Possible values:
  - `json`:
    XDR fields will be fetched as json and accessible via the headerJson and metadataJson fields
  - `xdr`:
    XDR fields will be fetched as xdr and accessible via the headerXdr and metadataXdr fields




## `stellar fee-stats`

Fetch network feestats

**Usage:** `stellar fee-stats [OPTIONS]`

###### **Options:**

* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--rpc-header <RPC_HEADERS>` — RPC Header(s) to include in requests to the RPC provider
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `-n`, `--network <NETWORK>` — Name of network to use from config
* `--output <OUTPUT>` — Format of the output

  Default value: `text`

  Possible values:
  - `text`:
    Text output of network info
  - `json`:
    JSON result of the RPC request
  - `json-formatted`:
    Formatted (multiline) JSON output of the RPC request




