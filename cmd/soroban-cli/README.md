# soroban-cli

CLI for interacting with the Stellar network and Soroban contracts locally in a test VM. Executes WASM files built using the [rs-soroban-sdk](https://github.com/stellar/rs-soroban-sdk).

Docs: https://developers.stellar.org

## Install

```
cargo install --locked stellar-cli
```

To install without features that depend on additional libraries:

```
cargo install --locked stellar-cli --no-default-features
```

## Usage

Can invoke a contract method as a subcommand with different arguments. Anything after the slop (`--`) is passed to the contract's CLI. You can use `--help` to learn about which methods are available and what their arguments are including an example of the type of the input.

## Example

```
stellar invoke --id <CONTRACT_ID> --wasm <WASMFILE> -- --help
stellar invoke --id <CONTRACT_ID> --network futurenet -- --help
```
