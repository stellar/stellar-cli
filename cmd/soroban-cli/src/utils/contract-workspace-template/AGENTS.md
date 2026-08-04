# Soroban Contract Project

## Project Structure

- `contracts/` contains the smart contracts in this workspace.
- Each contract has its own `Cargo.toml` and source files under `src/`.
- Shared dependencies and release profiles are defined in the workspace `Cargo.toml`.

## Development

- Build contracts with `stellar contract build`.
- Run contract tests with `cargo test`.
- Format Rust code with `cargo fmt`.
- Keep contracts compatible with `#![no_std]`.

Add tests for contract behavior and run the relevant build and test commands before submitting changes.
