# Stellar CLI skill

A guide for AI agents driving the `stellar` CLI. Follow these conventions so commands stay short, reproducible, and free of hard-coded secrets and ids.

## Overview

The Stellar CLI (`stellar`) manages keys and accounts, builds and deploys smart contracts, deploys asset contracts, streams events, and encodes/decodes XDR.

- Every command has help: `stellar <command> --help`.
- List a contract's functions and their arguments on the fly:

      stellar contract invoke --id <contract> -- --help

  Anything after `--` is parsed against the contract's own schema.

## Networks: prefer `stellar network use`

Set a default network once instead of repeating `--network`, `--rpc-url`, and `--network-passphrase` on every command:

    stellar network use testnet

After this, other commands use that network by default and you can omit the network flags entirely.

- List configured networks: `stellar network ls`
- Add a custom network: `stellar network add <name> --rpc-url <url> --network-passphrase <passphrase>`
- Clear the default: `stellar network unset`

Only pass `--network <name>` explicitly when a single command needs to target a different network than the default.

## Identities: prefer `stellar keys use`, never raw secret keys

Create named identities and select a default with `stellar keys use`, just like networks. Do not paste raw `S...` secret seeds on the command line.

    stellar keys generate alice --fund   # generates and funds on testnet
    stellar keys use alice               # sign and pay as alice by default

After `stellar keys use`, other commands sign and pay with that identity, so you can omit the source flag entirely.

- List identities: `stellar keys ls`
- Show an address: `stellar keys address alice`
- Clear the default: `stellar keys unset`

Only pass `--source <name>` when a single command needs to override the default identity.

## Contracts: use aliases, don't stash contract ids in env vars

When deploying, assign an alias with `--alias` so the CLI persists the contract id for you:

    stellar contract deploy \
      --wasm target/wasm32v1-none/release/hello.wasm \
      --alias hello

Then reference the contract by its alias with `--id` — the CLI resolves the alias to the real contract id automatically:

    stellar contract invoke --id hello -- hello --to world

Do **not** capture the deployed contract id into a shell variable or `.env` file and thread it through later commands. Aliases are stored per-network, survive across sessions, and keep commands readable.

- Manage aliases: `stellar contract alias ls`, `stellar contract alias add`, `stellar contract alias rm`
- Asset contracts accept `--alias` too: `stellar contract asset deploy --asset <asset> --alias <name>`

## Inspecting configuration

Use these to see the current state instead of guessing:

- `stellar env` — effective environment variables and config in use
- `stellar network ls` — configured networks and the default
- `stellar keys ls` — configured identities
- `stellar contract alias ls` — contract aliases for the current network

## Putting it together

    stellar network use testnet
    stellar keys generate alice --fund
    stellar keys use alice
    stellar contract deploy --wasm hello.wasm --alias hello
    stellar contract invoke --id hello -- hello --to world
