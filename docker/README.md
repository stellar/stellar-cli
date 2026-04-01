# Stellar CLI

Command-line interface for building and deploying smart contracts on the [Stellar](https://stellar.org) network.

For full documentation, visit [https://developers.stellar.org](https://developers.stellar.org).

## Quick Start

```sh
docker run --rm -it -v "$(pwd)":/source stellar/stellar-cli version
```

## Usage

The container expects your project files to be mounted at `/source` (the default working directory). Any `stellar` subcommand can be passed directly:

```sh
# Build a contract
docker run --rm -it -v "$(pwd)":/source stellar/stellar-cli contract build

# Deploy a contract
docker run --rm -it \
  -v "$(pwd)":/source \
  -e STELLAR_RPC_URL=https://soroban-testnet.stellar.org:443 \
  -e STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
  stellar/stellar-cli contract deploy --wasm target/wasm32v1-none/release/my_contract.wasm --source <key>
```

### Persisting Configuration

Configuration and data are stored inside the container by default and lost when it exits. Mount volumes to keep them across runs:

```sh
docker run --rm -it \
  -v "$(pwd)":/source \
  -v stellar-config:/config \
  -v stellar-data:/data \
  stellar/stellar-cli contract build
```

## Container Paths

| Path | Description |
| --- | --- |
| `/source` | Working directory where project files should be mounted. |
| `/config` | CLI configuration directory (`STELLAR_CONFIG_HOME`). Mount a volume to persist networks and keys across runs. |
| `/data` | CLI data directory (`STELLAR_DATA_HOME`). Mount a volume to persist cached contract specs and data. |

## Image Tags

- `latest` — most recent release.
- `X.Y.Z` — specific release version (e.g. `22.6.0`).
- `<commit-sha>` — build from a specific commit.
