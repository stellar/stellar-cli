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

## Building and deploying from source

Scaffold, build, and deploy a contract project:

    stellar contract init my-project     # scaffold a Cargo workspace
    cd my-project                        # enter the project before building
    stellar contract build               # compile to target/wasm32v1-none/release/<name>.wasm

Inside a contract project you can deploy without pointing at a `.wasm` — the CLI builds it for you:

    stellar contract deploy --alias my-project

If your contract defines a `__constructor`, forward its arguments after `--` as `--arg-name value` (e.g. `-- --admin alice`).

- Deploy a prebuilt file: `stellar contract deploy --wasm <path> --alias <name>`
- Upload Wasm without instantiating a contract (e.g. for factories or upgrades): `stellar contract upload` (the older `install` is a deprecated alias).

## Discovering a contract's interface

Besides `stellar contract invoke --id <c> -- --help`, you can inspect a deployed contract's functions and types without invoking it:

    stellar contract info interface --id <contract>

`--id` accepts a contract id or an alias and works across contract commands (it's the short form of `--contract-id`) — prefer it everywhere.

## Reading data and parsing output

- For view/query calls, use `--send=no` to simulate without submitting a transaction or paying fees:

      stellar contract invoke --id counter --send=no -- get_count

- A function's return value is printed to **stdout** as JSON; logs and diagnostics go to **stderr**. When capturing output for parsing, add `-q`/`--quiet` to silence logs and keep stdout clean.

## Data lifecycle and TTL (archival)

Contract storage is rented and each entry has a time-to-live (TTL). Behavior on expiry depends on the storage type: contract code, instance, and persistent entries are **archived** when their TTL runs out and must be restored before use, whereas **temporary** entries are deleted permanently and cannot be restored.

- `stellar contract read` — inspect a contract's storage entries
- `stellar contract extend` — bump an entry's TTL before it expires
- `stellar contract restore` — restore an archived persistent or instance entry (temporary data can't be restored)

## Running a local network

For fast, offline iteration, run a self-contained network (node + RPC + faucet) in a container:

    stellar container start local        # start the node + RPC (RPC on localhost:8000)
    stellar network use local            # point the CLI at it — otherwise commands keep using the previously selected network

- Container engine: defaults to Docker (or any Docker-compatible CLI such as Podman). On Apple silicon (macOS 26+) you can use Apple's `container` CLI. Set the default once with `stellar container use <engine>` (engines: `docker`, `apple-container`); override a single command with `--engine`, or set `STELLAR_CONTAINER_ENGINE`.
- Fund a local account: `stellar keys fund alice` (on testnet the same command funds via friendbot).

## Inspecting configuration

Use these to see the current state instead of guessing:

- `stellar env` — effective environment variables and config in use
- `stellar network ls` — configured networks and the default
- `stellar keys ls` — configured identities
- `stellar contract alias ls` — stored contract aliases, grouped by network

## Putting it together

    stellar network use testnet
    stellar keys generate alice --fund
    stellar keys use alice
    stellar contract deploy --wasm hello.wasm --alias hello
    stellar contract invoke --id hello -- hello --to world
