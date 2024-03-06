# Soroban CLI (soroban-cli)

This repo is home to the Soroban CLI, the command-line multi-tool for running and deploying Soroban contracts on the Stellar network.

## Documentation

For installation options see below, for usage instructions [see the manual](/docs/soroban-cli-full-docs.md).

## Install
Install the latest version from source:
```
cargo install --locked soroban-cli --features opt
```

Install with `cargo-binstall`:
```
cargo install --locked cargo-binstall
cargo binstall -y soroban-cli
```

Install with Homebrew:

```
brew install stellar/tap/soroban-cli
```

## Setup Autocomplete
```
soroban completion --shell <SHELL>
```
Possible SHELL values are `bash`, `elvish`, `fish`, `powershell`, `zsh`, etc.

To enable autocomplete in the current bash shell, run:
```
source <(soroban completion --shell bash)
```

To enable autocomplete permanently, run:
```
echo "source <(soroban completion --shell bash)" >> ~/.bashrc
```

## Latest Release
For latest releases, see [releases](https://github.com/stellar/soroban-cli/releases).

## Upcoming Features
For upcoming features, please see the [project board](https://github.com/orgs/stellar/projects/50).

## To Contribute
Find issues to contribute to [here](https://github.com/stellar/soroban-cli/contribute) and review [CONTRIBUTING.md](/CONTRIBUTING.md).

Developer Docs: https://developers.stellar.org/docs



