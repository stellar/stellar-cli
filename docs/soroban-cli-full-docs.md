# Command-Line Help for `soroban`

This document contains the help content for the `soroban` command-line program.

**Command Overview:**

* [`soroban`↴](#soroban)
* [`soroban contract`↴](#soroban-contract)
* [`soroban contract bindings`↴](#soroban-contract-bindings)
* [`soroban contract deploy`↴](#soroban-contract-deploy)
* [`soroban contract inspect`↴](#soroban-contract-inspect)
* [`soroban contract install`↴](#soroban-contract-install)
* [`soroban contract invoke`↴](#soroban-contract-invoke)
* [`soroban contract optimize`↴](#soroban-contract-optimize)
* [`soroban contract read`↴](#soroban-contract-read)
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
* `-f`, `--filter-logs <FILTER_LOGS>` — Filter logs output
* `-q`, `--quiet` — Do not write logs to stderr
* `-v`, `--verbose` — Log DEBUG events
* `--very-verbose` — Log DEBUG and TRACE events
* `--log-file <LOG_FILE>` — Write the output of the logs to a file
* `--list` — List installed plugins. E.g. `soroban-hello`



## `soroban contract`

Tools for smart contract developers

**Usage:** `soroban contract <COMMAND>`

###### **Subcommands:**

* `bindings` — Generate code client bindings for a contract
* `deploy` — Deploy a contract
* `inspect` — Inspect a WASM file listing contract functions, meta, etc
* `install` — Install a WASM file to the ledger without creating a contract instance
* `invoke` — Invoke a contract function
* `optimize` — Optimize a WASM file
* `read` — Print the current value of a contract-data ledger entry



## `soroban contract bindings`

Generate code client bindings for a contract

**Usage:** `soroban contract bindings --wasm <WASM> --output <OUTPUT>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary
* `--output <OUTPUT>` — Type of output to generate

  Possible values:
  - `rust`:
    Rust trait, client bindings, and test harness
  - `json`:
    Json representation of contract spec types




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



## `soroban contract inspect`

Inspect a WASM file listing contract functions, meta, etc

**Usage:** `soroban contract inspect --wasm <WASM>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary



## `soroban contract install`

Install a WASM file to the ledger without creating a contract instance

**Usage:** `soroban contract install [OPTIONS] --wasm <WASM>`

###### **Options:**

* `--wasm <WASM>` — Path to wasm binary
* `--rpc-url <RPC_URL>` — RPC server endpoint
* `--network-passphrase <NETWORK_PASSPHRASE>` — Network passphrase to sign the transaction sent to the rpc server
* `--network <NETWORK>` — Name of network to use from config
* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--source-account <SOURCE_ACCOUNT>` — Account that signs the final transaction. Alias `source`. Can be an identity (--source alice), a secret key (--source SC36…), or a seed phrase (--source "kite urban…"). Default: `identity generate --default-seed`
* `--hd-path <HD_PATH>` — If using a seed phrase, which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



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
* `--key-xdr <KEY_XDR>` — Storage key (base64-encoded XDR)
* `--output <OUTPUT>` — Type of output to generate

  Default value: `string`

  Possible values:
  - `string`:
    String
  - `json`:
    Json
  - `xdr`:
    XDR

* `--ledger-file <LEDGER_FILE>` — File to persist ledger state, default is `.soroban/ledger.json`
* `--global` — Use global config
* `--config-dir <CONFIG_DIR>`



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

  Possible values: `Value`, `ScpBallot`, `ScpStatementType`, `ScpNomination`, `ScpStatement`, `ScpStatementPledges`, `ScpStatementPrepare`, `ScpStatementConfirm`, `ScpStatementExternalize`, `ScpEnvelope`, `ScpQuorumSet`, `ScEnvMetaKind`, `ScEnvMetaEntry`, `ScSpecType`, `ScSpecTypeOption`, `ScSpecTypeResult`, `ScSpecTypeVec`, `ScSpecTypeMap`, `ScSpecTypeSet`, `ScSpecTypeTuple`, `ScSpecTypeBytesN`, `ScSpecTypeUdt`, `ScSpecTypeDef`, `ScSpecUdtStructFieldV0`, `ScSpecUdtStructV0`, `ScSpecUdtUnionCaseVoidV0`, `ScSpecUdtUnionCaseTupleV0`, `ScSpecUdtUnionCaseV0Kind`, `ScSpecUdtUnionCaseV0`, `ScSpecUdtUnionV0`, `ScSpecUdtEnumCaseV0`, `ScSpecUdtEnumV0`, `ScSpecUdtErrorEnumCaseV0`, `ScSpecUdtErrorEnumV0`, `ScSpecFunctionInputV0`, `ScSpecFunctionV0`, `ScSpecEntryKind`, `ScSpecEntry`, `ScValType`, `ScStatusType`, `ScHostValErrorCode`, `ScHostObjErrorCode`, `ScHostFnErrorCode`, `ScHostStorageErrorCode`, `ScHostAuthErrorCode`, `ScHostContextErrorCode`, `ScVmErrorCode`, `ScUnknownErrorCode`, `ScStatus`, `Int128Parts`, `ScContractExecutableType`, `ScContractExecutable`, `ScAddressType`, `ScAddress`, `ScVec`, `ScMap`, `ScBytes`, `ScString`, `ScSymbol`, `ScNonceKey`, `ScVal`, `ScMapEntry`, `StoredTransactionSet`, `PersistedScpStateV0`, `PersistedScpStateV1`, `PersistedScpState`, `Thresholds`, `String32`, `String64`, `SequenceNumber`, `DataValue`, `PoolId`, `AssetCode4`, `AssetCode12`, `AssetType`, `AssetCode`, `AlphaNum4`, `AlphaNum12`, `Asset`, `Price`, `Liabilities`, `ThresholdIndexes`, `LedgerEntryType`, `Signer`, `AccountFlags`, `SponsorshipDescriptor`, `AccountEntryExtensionV3`, `AccountEntryExtensionV2`, `AccountEntryExtensionV2Ext`, `AccountEntryExtensionV1`, `AccountEntryExtensionV1Ext`, `AccountEntry`, `AccountEntryExt`, `TrustLineFlags`, `LiquidityPoolType`, `TrustLineAsset`, `TrustLineEntryExtensionV2`, `TrustLineEntryExtensionV2Ext`, `TrustLineEntry`, `TrustLineEntryExt`, `TrustLineEntryV1`, `TrustLineEntryV1Ext`, `OfferEntryFlags`, `OfferEntry`, `OfferEntryExt`, `DataEntry`, `DataEntryExt`, `ClaimPredicateType`, `ClaimPredicate`, `ClaimantType`, `Claimant`, `ClaimantV0`, `ClaimableBalanceIdType`, `ClaimableBalanceId`, `ClaimableBalanceFlags`, `ClaimableBalanceEntryExtensionV1`, `ClaimableBalanceEntryExtensionV1Ext`, `ClaimableBalanceEntry`, `ClaimableBalanceEntryExt`, `LiquidityPoolConstantProductParameters`, `LiquidityPoolEntry`, `LiquidityPoolEntryBody`, `LiquidityPoolEntryConstantProduct`, `ContractDataEntry`, `ContractCodeEntry`, `ConfigSettingType`, `ConfigSetting`, `ConfigSettingId`, `ConfigSettingEntry`, `ConfigSettingEntryExt`, `LedgerEntryExtensionV1`, `LedgerEntryExtensionV1Ext`, `LedgerEntry`, `LedgerEntryData`, `LedgerEntryExt`, `LedgerKey`, `LedgerKeyAccount`, `LedgerKeyTrustLine`, `LedgerKeyOffer`, `LedgerKeyData`, `LedgerKeyClaimableBalance`, `LedgerKeyLiquidityPool`, `LedgerKeyContractData`, `LedgerKeyContractCode`, `LedgerKeyConfigSetting`, `EnvelopeType`, `UpgradeType`, `StellarValueType`, `LedgerCloseValueSignature`, `StellarValue`, `StellarValueExt`, `LedgerHeaderFlags`, `LedgerHeaderExtensionV1`, `LedgerHeaderExtensionV1Ext`, `LedgerHeader`, `LedgerHeaderExt`, `LedgerUpgradeType`, `LedgerUpgrade`, `LedgerUpgradeConfigSetting`, `BucketEntryType`, `BucketMetadata`, `BucketMetadataExt`, `BucketEntry`, `TxSetComponentType`, `TxSetComponent`, `TxSetComponentTxsMaybeDiscountedFee`, `TransactionPhase`, `TransactionSet`, `TransactionSetV1`, `GeneralizedTransactionSet`, `TransactionResultPair`, `TransactionResultSet`, `TransactionHistoryEntry`, `TransactionHistoryEntryExt`, `TransactionHistoryResultEntry`, `TransactionHistoryResultEntryExt`, `TransactionResultPairV2`, `TransactionResultSetV2`, `TransactionHistoryResultEntryV2`, `TransactionHistoryResultEntryV2Ext`, `LedgerHeaderHistoryEntry`, `LedgerHeaderHistoryEntryExt`, `LedgerScpMessages`, `ScpHistoryEntryV0`, `ScpHistoryEntry`, `LedgerEntryChangeType`, `LedgerEntryChange`, `LedgerEntryChanges`, `OperationMeta`, `TransactionMetaV1`, `TransactionMetaV2`, `ContractEventType`, `ContractEvent`, `ContractEventBody`, `ContractEventV0`, `DiagnosticEvent`, `OperationDiagnosticEvents`, `OperationEvents`, `TransactionMetaV3`, `TransactionMeta`, `TransactionResultMeta`, `TransactionResultMetaV2`, `UpgradeEntryMeta`, `LedgerCloseMetaV0`, `LedgerCloseMetaV1`, `LedgerCloseMetaV2`, `LedgerCloseMeta`, `ErrorCode`, `SError`, `SendMore`, `AuthCert`, `Hello`, `Auth`, `IpAddrType`, `PeerAddress`, `PeerAddressIp`, `MessageType`, `DontHave`, `SurveyMessageCommandType`, `SurveyMessageResponseType`, `SurveyRequestMessage`, `SignedSurveyRequestMessage`, `EncryptedBody`, `SurveyResponseMessage`, `SignedSurveyResponseMessage`, `PeerStats`, `PeerStatList`, `TopologyResponseBodyV0`, `TopologyResponseBodyV1`, `SurveyResponseBody`, `TxAdvertVector`, `FloodAdvert`, `TxDemandVector`, `FloodDemand`, `StellarMessage`, `AuthenticatedMessage`, `AuthenticatedMessageV0`, `LiquidityPoolParameters`, `MuxedAccount`, `MuxedAccountMed25519`, `DecoratedSignature`, `LedgerFootprint`, `OperationType`, `CreateAccountOp`, `PaymentOp`, `PathPaymentStrictReceiveOp`, `PathPaymentStrictSendOp`, `ManageSellOfferOp`, `ManageBuyOfferOp`, `CreatePassiveSellOfferOp`, `SetOptionsOp`, `ChangeTrustAsset`, `ChangeTrustOp`, `AllowTrustOp`, `ManageDataOp`, `BumpSequenceOp`, `CreateClaimableBalanceOp`, `ClaimClaimableBalanceOp`, `BeginSponsoringFutureReservesOp`, `RevokeSponsorshipType`, `RevokeSponsorshipOp`, `RevokeSponsorshipOpSigner`, `ClawbackOp`, `ClawbackClaimableBalanceOp`, `SetTrustLineFlagsOp`, `LiquidityPoolDepositOp`, `LiquidityPoolWithdrawOp`, `HostFunctionType`, `ContractIdType`, `ContractIdPublicKeyType`, `InstallContractCodeArgs`, `ContractId`, `ContractIdFromEd25519PublicKey`, `CreateContractArgs`, `HostFunction`, `AuthorizedInvocation`, `AddressWithNonce`, `ContractAuth`, `InvokeHostFunctionOp`, `Operation`, `OperationBody`, `HashIdPreimage`, `HashIdPreimageOperationId`, `HashIdPreimageRevokeId`, `HashIdPreimageEd25519ContractId`, `HashIdPreimageContractId`, `HashIdPreimageFromAsset`, `HashIdPreimageSourceAccountContractId`, `HashIdPreimageCreateContractArgs`, `HashIdPreimageContractAuth`, `MemoType`, `Memo`, `TimeBounds`, `LedgerBounds`, `PreconditionsV2`, `PreconditionType`, `Preconditions`, `TransactionV0`, `TransactionV0Ext`, `TransactionV0Envelope`, `Transaction`, `TransactionExt`, `TransactionV1Envelope`, `FeeBumpTransaction`, `FeeBumpTransactionInnerTx`, `FeeBumpTransactionExt`, `FeeBumpTransactionEnvelope`, `TransactionEnvelope`, `TransactionSignaturePayload`, `TransactionSignaturePayloadTaggedTransaction`, `ClaimAtomType`, `ClaimOfferAtomV0`, `ClaimOfferAtom`, `ClaimLiquidityAtom`, `ClaimAtom`, `CreateAccountResultCode`, `CreateAccountResult`, `PaymentResultCode`, `PaymentResult`, `PathPaymentStrictReceiveResultCode`, `SimplePaymentResult`, `PathPaymentStrictReceiveResult`, `PathPaymentStrictReceiveResultSuccess`, `PathPaymentStrictSendResultCode`, `PathPaymentStrictSendResult`, `PathPaymentStrictSendResultSuccess`, `ManageSellOfferResultCode`, `ManageOfferEffect`, `ManageOfferSuccessResult`, `ManageOfferSuccessResultOffer`, `ManageSellOfferResult`, `ManageBuyOfferResultCode`, `ManageBuyOfferResult`, `SetOptionsResultCode`, `SetOptionsResult`, `ChangeTrustResultCode`, `ChangeTrustResult`, `AllowTrustResultCode`, `AllowTrustResult`, `AccountMergeResultCode`, `AccountMergeResult`, `InflationResultCode`, `InflationPayout`, `InflationResult`, `ManageDataResultCode`, `ManageDataResult`, `BumpSequenceResultCode`, `BumpSequenceResult`, `CreateClaimableBalanceResultCode`, `CreateClaimableBalanceResult`, `ClaimClaimableBalanceResultCode`, `ClaimClaimableBalanceResult`, `BeginSponsoringFutureReservesResultCode`, `BeginSponsoringFutureReservesResult`, `EndSponsoringFutureReservesResultCode`, `EndSponsoringFutureReservesResult`, `RevokeSponsorshipResultCode`, `RevokeSponsorshipResult`, `ClawbackResultCode`, `ClawbackResult`, `ClawbackClaimableBalanceResultCode`, `ClawbackClaimableBalanceResult`, `SetTrustLineFlagsResultCode`, `SetTrustLineFlagsResult`, `LiquidityPoolDepositResultCode`, `LiquidityPoolDepositResult`, `LiquidityPoolWithdrawResultCode`, `LiquidityPoolWithdrawResult`, `InvokeHostFunctionResultCode`, `InvokeHostFunctionResult`, `OperationResultCode`, `OperationResult`, `OperationResultTr`, `TransactionResultCode`, `InnerTransactionResult`, `InnerTransactionResultResult`, `InnerTransactionResultExt`, `InnerTransactionResultPair`, `TransactionResult`, `TransactionResultResult`, `TransactionResultExt`, `Hash`, `Uint256`, `Uint32`, `Int32`, `Uint64`, `Int64`, `TimePoint`, `Duration`, `ExtensionPoint`, `CryptoKeyType`, `PublicKeyType`, `SignerKeyType`, `PublicKey`, `SignerKey`, `SignerKeyEd25519SignedPayload`, `Signature`, `SignatureHint`, `NodeId`, `AccountId`, `Curve25519Secret`, `Curve25519Public`, `HmacSha256Key`, `HmacSha256Mac`

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
