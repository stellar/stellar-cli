# Stellar CLI (stellar-cli)

![Apache 2.0 licensed](https://img.shields.io/badge/license-apache%202.0-blue.svg)
[![Crates.io Version](https://img.shields.io/crates/v/stellar-cli?label=version&amp;color=04ac5b)](https://crates.io/crates/stellar-cli)

This repo is home to the Stellar CLI, the command-line multi-tool for running and deploying Stellar contracts on the Stellar network.


## Table of Contents

- [Documentation](#documentation)
- [Cookbook](#cookbook)
- [Installation](#installation)
- [Installation with Experimental Features](#installation-with-experimental-features)
- [Autocomplete](#autocomplete)
- [Latest Release](#latest-release)
- [Upcoming Features](#upcoming-features)
- [To Contribute](#to-contribute)
- [Additional Developer Resources](#additional-developer-resources)



## Documentation

For installation options see below, for usage instructions [see the full help docs](FULL_HELP_DOCS.md).

## Cookbook
To understand how to get the most of the Stellar CLI, see the [Stellar CLI Cookbook](https://github.com/stellar/stellar-cli/tree/main/cookbook) for recipes and a collection of resources to teach you how to use the CLI. Examples of recipes included in the CLI cookbook include: send payments, manage contract lifecycle, extend contract instance/storage/wasm, and more.

## Installation
Install the latest version from source:
```
cargo install --locked stellar-cli --features opt
```

Install with `cargo-binstall`:
```
cargo install --locked cargo-binstall
cargo binstall -y stellar-cli
```

Install with Homebrew:

```
brew install stellar-cli
```

## Installation with Experimental Features
To use the potentially unreleased bleeding edge CLI functionalities, install from git:
```
cargo install --locked stellar-cli --features opt --git https://github.com/stellar/stellar-cli.git
```

## Autocomplete
The Stellar CLI supports some autocompletion. To set up, run the following commands:

```
stellar completion --shell <SHELL>
```
Possible SHELL values are `bash`, `elvish`, `fish`, `powershell`, `zsh`, etc.

To enable autocomplete in the current bash shell, run:
```
source <(stellar completion --shell bash)
```

To enable autocomplete permanently, run:
```
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
