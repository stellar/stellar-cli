# stellar-contract-cli

CLI for running Stellar contracts locally in a test VM. Executes WASM files built using the [rs-stellar-contract-sdk](https://github.com/stellar/rs-stellar-contract-sdk).

## Install

```
cargo install stellar-contract-cli --git https://github.com/stellar/stellar-contract-cli
```

## Usage

All values passed to `--arg` are the JSON representation of SCVals.

### Directly invoking a function in a user specified WASM contract

```
stellar-contract-cli invoke --file <WASMFILE> vm-fn --fn <FUNCNAME> --arg '{"i32":32}' --arg '{"i32":4}'
```

Example using the [example_add_i32](https://github.com/stellar/rs-stellar-contract-sdk/tree/main/examples/add_i32) contract:

```
$ stellar-contract-cli invoke --file example_add_i32.wasm --fn add --arg '{"i32":32}' --arg '{"i32":4}'

{
  "i32": 36
}
```

### Creating and invoking a contract in storage

#### Creating a contract

```
Example:
stellar-contract-cli deploy --id 3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c --file example_add_i32.wasm
```

#### Invoking an existing contract

```
stellar-contract-cli invoke --fn add --id <HEX_CONTRACTID>  --arg '{"i32":32}' --arg '{"i32":4}'

Example:
stellar-contract-cli invoke --fn add --id 3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c --arg '{"i32":32}' --arg '{"i32":4}'


stellar-contract-cli invoke --fn put --id 3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c --arg '{"symbol": [69, 50, 53, 49, 57]}' --arg '{"symbol": [69, 50, 53]}'
```
