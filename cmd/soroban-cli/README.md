# soroban-cli

CLI for running Soroban contracts locally in a test VM. Executes WASM files built using the [rs-soroban-sdk](https://github.com/stellar/rs-soroban-sdk).

Soroban: https://soroban.stellar.org

## Install

```
cargo install --locked soroban-cli
```

To install with the `opt` feature, which includes a WASM optimization feature and wasm-opt built in:

```
cargo install --locked soroban-cli --features opt
```

## Usage

Can invoke a contract method as a subcommand with different arguments. Anything after the slop (`--`) is passed to the contract's CLI. You can use `--help` to learn about which methods are available and what their arguments are including an example of the type of the input.

## Example

```
soroban invoke --id <CONTRACT_ID> --wasm <WASMFILE> -- --help
soroban invoke --id <CONTRACT_ID> --network futurenet -- --help
```
