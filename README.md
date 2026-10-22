# Stellar CLI (stellar-cli)

[![Apache 2.0 licensed](https://img.shields.io/badge/license-apache%202.0-blue.svg)](LICENSE) [![Crates.io Version](https://img.shields.io/crates/v/stellar-cli?label=version&color=04ac5b)](https://crates.io/crates/stellar-cli) [![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/stellar/stellar-cli)

This repo is home to the Stellar CLI, the command-line multi-tool for running and deploying Stellar contracts on the Stellar network.

## Table of Contents

- [Documentation](#documentation)
- [Cookbook](#cookbook)
- [Install](#install)
- [Autocomplete](#autocomplete)
- [Security Considerations](#security-considerations)
- [Latest Release](#latest-release)
- [Upcoming Features](#upcoming-features)
- [To Contribute](#to-contribute)
- [Additional Developer Resources](#additional-developer-resources)

## Documentation

For installation options see below, for usage instructions [see the full help docs](FULL_HELP_DOCS.md).

## Cookbook

To understand how to get the most of the Stellar CLI, see the [Stellar CLI Cookbook](https://github.com/stellar/stellar-cli/tree/main/cookbook) for recipes and a collection of resources to teach you how to use the CLI. Examples of recipes included in the CLI cookbook include: send payments, manage contract lifecycle, extend contract instance/storage/wasm, and more.

## Install

Install with the install script (macOS, Linux):

```sh
curl -fsSL https://github.com/stellar/stellar-cli/raw/main/install.sh | sh
```

For installation options, run the script with `--help`:

```sh
curl -fsSL https://github.com/stellar/stellar-cli/raw/main/install.sh | sh -s -- --help
```

Install with dependency setup (`--install-deps`) to attempt installing:

- Linux development dependencies (`libdbus`/`libudev` development packages; Linux only, not required on macOS)
- Rust toolchain and `wasm32v1-none` target (for contract build workflows)

```sh
curl -fsSL https://github.com/stellar/stellar-cli/raw/main/install.sh | sh -s -- --install-deps
```

Install with Homebrew (macOS, Linux):

```sh
brew install stellar-cli
```

Install the latest version from source:

```sh
cargo install --locked stellar-cli
```

Install without features that depend on additional libraries:

```sh
cargo install --locked stellar-cli --no-default-features
```

Install or run the unreleased main branch with nix:

```console
$ nix run 'github:stellar/stellar-cli' -- --help
or install
$ nix profile install github:stellar/stellar-cli
```

For additional information on how to install, see instructions here on the [Developer Docs](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup#install).

Use GitHub Action:

```yaml
uses: stellar/stellar-cli@v23.0.1
```

Run with Docker:

```sh
docker run --rm -it -v "$(pwd)":/source stellar/stellar-cli version
```

Docker image distribution is handled by the [stellar/stellar-cli-docker](https://github.com/stellar/stellar-cli-docker) repository, with images published to [Docker Hub](https://hub.docker.com/r/stellar/stellar-cli/).

## Autocomplete

The Stellar CLI supports some autocompletion. To set up, run the following commands:

```sh
stellar completion --shell <SHELL>
```

Possible SHELL values are `bash`, `elvish`, `fish`, `powershell`, `zsh`, etc.

To enable autocomplete in the current bash shell, run:

```bash
source <(stellar completion --shell bash)
```

To enable autocomplete permanently, run:

```bash
echo "source <(stellar completion --shell bash)" >> ~/.bashrc
```

## Security Considerations

### RPC Trust Boundary

The Stellar CLI relies on the configured Stellar RPC endpoint (`--rpc-url` or configured network endpoint) to prepare, simulate, and submit transactions. Users and integrators should be aware of the following security model:

- **Simulation and Authorization Entries**: When invoking smart contracts, the CLI relies on the RPC endpoint's `simulateTransaction` response to generate the contract footprint (read/write ledger keys) and assemble required authorization entries (`SorobanAuthorizationEntry`).
- **Transaction Parameters and Lifetimes**: The CLI derives transaction parameters and ledger bounds—such as sequence numbers, network passphrases, and `signature_expiration_ledger`—from RPC queries (such as `getLatestLedger`).

Because the CLI treats the RPC endpoint as a trusted component in this pipeline:
- **Untrusted RPC Risks**: Directing the CLI to an untrusted or compromised RPC endpoint may result in malicious simulation results, crafted or overprivileged authorization entries, or manipulated transaction contents being presented for signing.
- **Unencrypted Transport (Plain HTTP)**: Using plain `http://` RPC endpoints leaves communication vulnerable to eavesdropping and man-in-the-middle (MITM) tampering, allowing attackers on the network path to modify simulation outputs or transaction parameters.

**Best Practices & Guidance**:
1. **Always Use TLS/HTTPS**: Connect to RPC endpoints over `https://` with valid, trusted certificates to prevent transport-level tampering.
2. **Use Controlled or Reputable Endpoints**: Point the CLI only to RPC nodes you control and operate, or to well-known, reputable infrastructure providers (e.g., official Stellar Development Foundation RPCs).
3. **Verify Auth Entries Before Signing**: When signing transactions with high-privilege keys, inspect the authorization entries and transaction parameters reported by the CLI (or preview them with `--send=no` / `--build-only`) before signing.

## Latest Release

For the latest release, see [releases](https://github.com/stellar/stellar-cli/releases).

## Upcoming Features

For upcoming features, please see the [project board](https://github.com/orgs/stellar/projects/50).

## To Contribute

Find issues to contribute to [here](https://github.com/stellar/stellar-cli/contribute) and review [CONTRIBUTING.md](/CONTRIBUTING.md).

## Additional Developer Resources

- Developer Docs CLI Examples: https://developers.stellar.org/docs/smart-contracts/guides/cli
- Video Tutorial on `network container`, `keys`, and `contract init`: https://developers.stellar.org/meetings/2024/06/27
- Video Tutorial on `alias` and `snapshot`: https://developers.stellar.org/meetings/2024/09/12
