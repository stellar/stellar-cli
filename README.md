# soroban-tools

This repo is home to the suite of Soroban development tools.
- [soroban](https://github.com/stellar/soroban-tools/tree/main/cmd/soroban-cli): The command-line multi-tool for running and deploying Soroban contracts.
- [soroban-rpc](https://github.com/stellar/soroban-tools/tree/main/cmd/soroban-rpc): The jsonrpc server for interacting with a running Soroban network.

Soroban: https://soroban.stellar.org

# linting
 
Before submitting a PR for review, please run

```
make lint-changes
```

to review all the linting issues with the PR. Alternatively, you can run

```
make lint
```

to review all the linting issues with the current codebase.

# Adding git hooks

To add git hooks for commits and pushes run:

```
./install_githooks.sh
```

which copies the git hooks found at `.cargo-husky/hooks` to `.git/hooks`.