# Stellar Ledger

This crate allows for interaction with Ledger devices, and exposes the following functions:

- `get_app_configuration`
- `get_public_key`
- `sign_transaction_hash`
- `sign_transaction`
- `sign_blob`

## Tests

There are several unit tests in lib.rs, as well as integration-like tests in the emulator_tests.rs file. emulator_tests.rs uses [testcontainers-rs](https://github.com/testcontainers/testcontainers-rs) to spin up a docker container running a Ledger emulator called [Speculos](https://github.com/LedgerHQ/speculos).

## Resources

- [LedgerHQ/ledger-live/../hw-app-str](https://github.com/LedgerHQ/ledger-live/tree/develop/libs/ledgerjs/packages/hw-app-str) is javascript implementation of the API for interacting with the Stellar app on Ledger devices. We used this as a reference when building the `stellar-ledger` crate.
- The communication protocol used by Ledger devices expects commands to be sent as Application Protocol Data Units (APDU).
  - More information about how APDUs are structured can be found here [https://github.com/LedgerHQ/app-stellar/blob/develop/docs/APDU.md](https://github.com/LedgerHQ/app-stellar/blob/develop/docs/APDU.md).
  - The list of commands that the Stellar App on Ledger devices currently supports can be found here [https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md](https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md).
- The Ledger emulator we're using for integration-style tests is LedgerHQ's [Speculos](https://github.com/LedgerHQ/speculos).
- The testing setup was also partially based on Zondax's [Zemu](https://github.com/Zondax/zemu) testing framework, which makes use of Speculos.
- To connect with a real ledger device, we use Zondax's [ledger-rs](https://github.com/Zondax/ledger-rs) crate.
- To connect with the emulated ledger (Speculos), we created a custom `EmulatorHttpTransport` that can connect to the emulated ledger via HTTP. This is based on [Zondax's `ledger-transport-zemu` crate](https://github.com/Zondax/ledger-rs/blob/20e2a2076d799d449ff6f07eb0128548b358d9bc/ledger-transport-zemu) (which has since been deprecated).
