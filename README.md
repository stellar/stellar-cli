# soroban-cli

CLI for running Soroban contracts locally in a test VM. Executes WASM files built using the [rs-soroban-sdk](https://github.com/stellar/rs-soroban-sdk).

Soroban: https://soroban.stellar.org

## Install

```
cargo install --locked soroban-cli
```

## Usage

All values passed to `--arg` are the JSON representation of SCVals.

## Example

```
soroban-cli invoke --id <HEX_CONTRACTID> --wasm <WASMFILE> --fn <FUNCNAME> --arg 32 --arg 4
```
