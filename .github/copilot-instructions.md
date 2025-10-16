# Stellar CLI Development Instructions

**Always follow these instructions first and fallback to additional search and context gathering only when the information here is incomplete or found to be in error.**

## Working Effectively

Stellar CLI is a Rust-based command-line tool for interacting with the Stellar network. It's organized as a Cargo workspace with multiple crates and uses both Cargo and Make for build automation.

### Bootstrap and Build

- Install system dependencies: `sudo apt-get update && sudo apt-get install -y libudev-dev libdbus-1-dev build-essential`
- Install Rust toolchain: `rustup update` (Rust 1.89.0+ required)
- Add WebAssembly target: `rustup target add wasm32v1-none`
- Build main CLI: `cargo build --bin stellar` -- takes 45 seconds. NEVER CANCEL.
- Install CLI: `make install` -- takes 3 minutes with potential network timeouts. NEVER CANCEL. Set timeout to 10+ minutes.

### Core Development Commands

- Format code: `make fmt` -- takes 2 seconds
- Run linting: `make check` -- takes 7 minutes. NEVER CANCEL. Set timeout to 15+ minutes.
- Build main CLI only: `cargo build --bin stellar` -- takes 45 seconds. Use this for quick iterations.

### Testing

- Test main soroban-cli library: `cargo test --package soroban-cli --lib` -- takes 52 seconds. NEVER CANCEL.
- Test individual crates: `cargo test --package <crate-name>` -- typically takes 40 seconds per crate.
- Test soroban-test integration tests: `cargo test --features it --test it -- integration` -- tests the commands of the cli and is where the bulk of the tests live for this repository. All new commands and changes to commands should include updates or additions to tests in soroban-test.
- **WARNING**: Full test suite via `make test` requires building WebAssembly test fixtures and consumes significant memory and disk space. It may fail with "No space left on device" in constrained environments.

### CLI Usage and Validation

- Test CLI installation: `stellar --version`
- Basic CLI validation: `stellar --help`
- Generate test keys: `stellar keys generate <name>`
- Get key address: `stellar keys address <name>`

## Validation

- **ALWAYS run basic validation after changes**: Build the CLI with `cargo build --bin stellar` and test basic functionality with `stellar --help`.
- **Core testing workflow**: Run `cargo test --package soroban-cli --lib` to validate core functionality.
- **Pre-commit validation**: Always run `make fmt` and `make check` before committing changes.
- **Memory considerations**: Full test suite may fail in constrained environments due to memory and disk space requirements.

## Common Tasks

### Repository Structure

```
/home/runner/work/stellar-cli/stellar-cli/
├── cmd/
│   ├── stellar-cli/          # Main CLI binary package
│   ├── soroban-cli/          # Core CLI functionality
│   └── crates/               # Supporting crates
│       ├── soroban-spec-tools/
│       ├── soroban-spec-typescript/
│       ├── soroban-test/
│       └── stellar-ledger/
├── Makefile                  # Build automation
├── Cargo.toml               # Workspace configuration
└── .github/workflows/       # CI configuration
```

### Key Commands Reference

```bash
# Development workflow
cargo build --bin stellar        # Quick build (45s)
make install                     # Full install (3m, may timeout)
make fmt                         # Format code (2s)
make check                       # Lint code (7m)

# Testing
cargo test --package soroban-cli --lib  # Core tests (52s)
cargo test --package <crate>            # Individual crate tests (~40s)

# CLI validation
stellar --version               # Check installation
stellar --help                 # Verify functionality
stellar keys generate test     # Test key generation
stellar keys address test      # Test key operations
```

### Build Time Expectations

- **NEVER CANCEL** any build or test command before these timeouts:
  - `cargo build --bin stellar`: 2 minutes timeout minimum
  - `make install`: 10 minutes timeout (network issues common)
  - `make check`: 15 minutes timeout minimum
  - Individual crate tests: 5 minutes timeout minimum
  - Core library tests: 5 minutes timeout minimum

### Known Issues

- **Network timeouts**: `make install` frequently encounters network timeouts with crates.io. This is normal in CI environments.
- **Memory constraints**: Full workspace build and test may fail with OOM or "No space left on device" errors in constrained environments.
- **Documentation generation**: `make docs` may fail due to disk space constraints.

### Working Around Constraints

- Use `cargo build --bin stellar` instead of full workspace build for development
- Test individual packages with `cargo test --package <name>` instead of full test suite
- Use `cargo install --force --locked --path ./cmd/stellar-cli` as alternative to `make install`

## Critical Warnings

- **NEVER CANCEL builds or tests** before the specified timeout periods
- **Always validate changes** by building and testing the CLI before committing
- **Network issues are common** - retry `make install` if it fails with timeouts
- **Use surgical builds** when possible to avoid memory/disk issues

## CI/CD Integration

The project uses GitHub Actions with workflows in `.github/workflows/`:

- `rust.yml`: Main CI pipeline with formatting, linting, building, and testing
- `e2e.yml`: End-to-end system tests
- `binaries.yml`: Multi-platform binary builds

Always run `make fmt` and `make check` locally before pushing to ensure CI passes.
