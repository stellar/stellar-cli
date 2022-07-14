# stellar-contract-cli

CLI for running Stellar contracts locally in a test VM. Executes WASM files built using the [rs-stellar-contract-sdk](https://github.com/stellar/rs-stellar-contract-sdk).

## Install

```
cargo install stellar-contract-cli --git https://github.com/stellar/stellar-contract-cli
```

## Usage

### Directly invoking a function in auser specified WASM contract

```
stellar-contract-cli invoke --file <WASMFILE> vm-fn --fn <FUNCNAME> --arg i32:1 --arg i32:2
```

Example using the [example_add_i32](https://github.com/stellar/rs-stellar-contract-sdk/tree/main/examples/add_i32) contract:

```
$ stellar-contract-cli invoke --file example_add_i32.wasm --fn add --arg i32:1 --arg i32:2
i32:3
```

### Calling a host function

For both options below, the file should contain a JSON representation of the `SCVec` xdr object, which is passed directly to `invoke_function` in the host. The order of the items in the `SCVec` is specified below.

#### Creating a contract

```
// args = SCVec(contractCode, salt, ed25519, signature)
stellar-contract-cli invoke --file <JSONARGS> create-contract
```

The input file is the 

#### Invoking an existing contract

```
// args = SCVec(contractID, functionName, args...)
stellar-contract-cli invoke --file <JSONARGS> call
```
