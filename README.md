# soroban-tools

This repo is home to the [Soroban CLI](https://github.com/stellar/soroban-tools/tree/main/cmd/soroban-cli): The command-line multi-tool for running and deploying Soroban contracts.

Soroban: https://soroban.stellar.org

# Adding git hooks

To add git hooks for commits and pushes run:

```
./install_githooks.sh
```

which copies the git hooks found at `.cargo-husky/hooks` to `.git/hooks`.