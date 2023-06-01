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

All values passed to `--arg` are the JSON representation of SCVals.

## Example

```
soroban invoke --id <HEX_CONTRACTID> --wasm <WASMFILE> -- <FUNCNAME> --<contract fn argument name> <value>
```
