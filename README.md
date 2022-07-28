# soroban-cli

CLI for running Soroban contracts locally in a test VM. Executes WASM files built using the [rs-soroban-sdk](https://github.com/stellar/rs-soroban-sdk).

## Install

```
cargo install soroban-cli --git https://github.com/stellar/soroban-cli
```

## Usage

All values passed to `--arg` are the JSON representation of SCVals.

### Creating a contract and invoking a function
Note that this will also put the WASM contract in storage.

```
soroban-cli invoke --id <HEX_CONTRACTID> --file <WASMFILE> --fn <FUNCNAME> --arg '{"i32":32}' --arg '{"i32":4}'
```

Example using the [example_add_i32](https://github.com/stellar/rs-soroban-sdk/tree/main/examples/add_i32) contract:

```
$ soroban-cli invoke --id 3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c --file example_add_i32.wasm --fn add --arg '{"i32":32}' --arg '{"i32":4}'

{
  "i32": 36
}
```

### Creating a contract

```
Example:
soroban-cli deploy --id 3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c --file example_add_i32.wasm
```

### Invoking a contract in storage

```
soroban-cli invoke --fn add --id <HEX_CONTRACTID>  --arg '{"i32":32}' --arg '{"i32":4}'

Example:
soroban-cli invoke --fn add --id 3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c --arg '{"i32":32}' --arg '{"i32":4}'


soroban-cli invoke --fn put --id 3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c --arg '{"symbol": [69, 50, 53, 49, 57]}' --arg '{"symbol": [69, 50, 53]}'
```
