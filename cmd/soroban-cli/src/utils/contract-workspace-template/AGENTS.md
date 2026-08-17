# Agent instructions

This is a Stellar smart-contract workspace (Soroban). Each contract is a
workspace member under `contracts/<name>/`.

## Layout

- `Cargo.toml` — workspace root; contract crates inherit `soroban-sdk` from here
- `contracts/<name>/src/lib.rs` — contract implementation (`#![no_std]`)
- `contracts/<name>/src/test.rs` — host-side unit tests

## Build

From the workspace root:

```sh
stellar contract build
```

That compiles every `cdylib` member to WASM. Artifacts land in
`target/wasm32v1-none/release/*.wasm`. Build one crate with
`stellar contract build --package <name>`.

Do not substitute this with `cargo build --target wasm32v1-none`. `stellar contract build`
applies the flags and metadata the network expects.

The `wasm32v1-none` Rust target must be installed (`rustup target add wasm32v1-none`).
Rust 1.84 or newer is required for that target. Rust 1.82 and 1.83 cannot build
contracts.

## Test

Host tests run with the normal Cargo test harness (not on-chain):

```sh
cargo test
```

A single crate: `cargo test -p <name>`.

## Deploy and invoke

On testnet, after a successful build:

```sh
stellar contract deploy \
  --wasm target/wasm32v1-none/release/<name>.wasm \
  --source-account <identity> \
  --network testnet \
  --alias <alias>

stellar contract invoke \
  --id <alias> \
  --network testnet \
  --source-account <identity> \
  -- hello --to world
```

The sample `hello_world` contract exposes `hello(to: String) -> Vec<String>`.
Replace that with your own functions; `stellar contract invoke --id <id> -- -h`
prints the generated CLI for the deployed contract.

## Further reading

- https://developers.stellar.org/docs/build/smart-contracts/overview
- https://github.com/stellar/soroban-examples
