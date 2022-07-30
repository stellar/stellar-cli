all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic

install:
	cargo install --path .

test:
	cargo test

build:
	cargo build

check:
	cargo clippy --all-targets

watch:
	cargo watch --clear --watch-when-idle --shell '$(MAKE)'

fmt:
	cargo fmt --all

clean:
	cargo clean

# Build all projects as if they are being published to crates.io, and do so for
# all feature and target combinations.
publish-dry-run:
	cargo +stable hack --feature-powerset publish --locked --dry-run --package soroban-cli

# Publish publishes the crate to crates.io. The dry-run is a dependency because
# the dry-run target will verify all feature set combinations.
publish: publish-dry-run
	cargo +stable publish --locked --package soroban-cli
