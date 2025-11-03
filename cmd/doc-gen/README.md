# Stellar CLI Documentation Generator

A generator for the help documentation using clap-markdown.

## Usage

Run the following command from the workspace root.

```
cargo run --package doc-gen
```

The command will update the file `FULL_HELP_DOCS.md` located in the root of the workspace.

Typically this command is not run directly, but in the root `Makefile` the `docs` target will run this command in conjunction with formatting the markdown.

So typically run this command via make:

```
make docs
```
