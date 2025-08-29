# Stellar CLI (stellar-cli)

[![Apache 2.0 licensed](https://img.shields.io/badge/license-apache%202.0-blue.svg)](LICENSE)
[![Crates.io Version](https://img.shields.io/crates/v/stellar-cli?label=version&amp;color=04ac5b)](https://crates.io/crates/stellar-cli)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/stellar/stellar-cli)

This repo is home to the Stellar CLI, the command-line multi-tool for running and deploying Stellar contracts on the Stellar network.


## Table of Contents

- [Documentation](#documentation)
- [Cookbook](#cookbook)
- [Install](#install)
- [Autocomplete](#autocomplete)
- [Latest Release](#latest-release)
- [Upcoming Features](#upcoming-features)
- [To Contribute](#to-contribute)
- [Additional Developer Resources](#additional-developer-resources)

## Documentation

For installation options see below, for usage instructions [see the full help docs](FULL_HELP_DOCS.md).

## Cookbook
To understand how to get the most of the Stellar CLI, see the [Stellar CLI Cookbook](https://github.com/stellar/stellar-cli/tree/main/cookbook) for recipes and a collection of resources to teach you how to use the CLI. Examples of recipes included in the CLI cookbook include: send payments, manage contract lifecycle, extend contract instance/storage/wasm, and more.

## Install

Install with Homebrew (macOS, Linux):

```
brew install stellar-cli
```

Install the latest version from source:
```
cargo install --locked stellar-cli
```

Install without features that depend on additional libraries:
```
cargo install --locked stellar-cli --no-default-features
```

Install or run the unreleased main branch with nix:
```
$ nix run 'github:stellar/stellar-cli' -- --help
or install
$ nix profile install github:stellar/stellar-cli
```

For additional information on how to install, see instructions here on the [Developer Docs](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup#install). 

Use GitHub Action:
```
uses: stellar/stellar-cli@v23.0.1
```

## Autocomplete
The Stellar CLI supports some autocompletion. To set up, run the following commands:

```
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

<!-- CI system test comment -->

