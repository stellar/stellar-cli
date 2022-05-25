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
