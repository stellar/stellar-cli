# Payments and Assets

## Send XLM, stellar classic, or a soroban asset using the Stellar CLI.

To send payments and work with assets using the Stellar CLI, follow these steps:

1. Set your preferred network. For this guide, we will use `testnet`. A list of available networks can be found [here](https://developers.stellar.org/docs/networks)

```bash
export STELLAR_NETWORK=testnet
```

By setting the `STELLAR_NETWORK` environment variable, we will not have to set the `--network` argument when using the CLI.

2. Fund the accounts:

```bash
stellar keys generate alice --no-fund
stellar keys generate bob --no-fund
stellar keys fund alice
stellar keys fund bob
```

3. Obtain the stellar asset contract ID:

```bash
stellar contract id asset --asset native --source-account alice
```

4. Get Bob's public key:

```bash
stellar keys address bob
```

5. Send 100 XLM from Alice to Bob:

```bash
stellar contract invoke --id <asset contract ID> --source-account alice  -- transfer --to <Bob ID> --from alice --amount 100
```

6. Check account balance:

```bash
stellar contract invoke --id <asset contract ID> --source-account alice  -- balance --id <account ID>
```

For more information on the functions available to the stellar asset contract, see the [token interface code](https://developers.stellar.org/docs/tokens/token-interface#code).

# Contract Lifecycle

## Manage the lifecycle of a Stellar smart contract using the CLI.

To manage the lifecycle of a Stellar smart contract using the CLI, follow these steps:

1. Create an identity for Alice:

```bash
stellar keys generate alice --no-fund
```

2. Fund the identity:

```bash
stellar keys fund alice
```

3. Deploy a contract:

```bash
stellar contract deploy --wasm /path/to/contract.wasm --source alice --network testnet
```

This will display the resulting contract ID, e.g.:

```
CBB65ZLBQBZL5IYHDHEEPCVUUMFOQUZSQKAJFV36R7TZETCLWGFTRLOQ
```

To learn more about how to build contract `.wasm` files, take a look at our [getting started tutorial](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup).

4. Initialize the contract:

```bash
stellar contract invoke --id <CONTRACT_ID> --source alice --network testnet -- initialize --param1 value1 --param2 value2
```

5. Invoke a contract function:

```bash
stellar contract invoke --id <CONTRACT_ID> --source alice --network testnet -- function_name --arg1 value1 --arg2 value2
```

6. View the contract's state:

```bash
stellar contract read --id <CONTRACT_ID> --network testnet --source alice --durability <DURABILITY> --key <KEY>
```

Note: `<DURABILITY>` is either `persistent` or `temporary`. `KEY` provides the key of the storage entry being read.

7. Manage expired states:

```bash
stellar contract extend --id <CONTRACT_ID> --ledgers-to-extend 1000 --source alice --network testnet --durability <DURABILITY> --key <KEY>
```

This extends the state of the instance provided by the given key to at least 1000 ledgers from the current ledger.
