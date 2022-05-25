# stellar-contract-cli

CLI for running Stellar contracts locally in a test VM. Executes WASM files built using the [rs-stellar-contract-sdk](https://github.com/stellar/rs-stellar-contract-sdk).

## Install

```
cargo install stellar-contract-cli --git https://github.com/stellar/stellar-contract-cli
```

## Usage

```
stellar-contract-cli invoke --file <WASMFILE> --fn <FUNCNAME> --arg i32:1 --arg i32:2
```

Example using the [example_add_i32](https://github.com/stellar/rs-stellar-contract-sdk/tree/main/examples/add_i32) contract:

```
$ stellar-contract-cli invoke --file example_add_i32.wasm --fn add --arg i32:1 --arg i32:2
i32:3
```
