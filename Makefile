all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic

install:
	cargo install --path .

build:
	cargo build

build-test-wasms:
	cargo build --workspace --exclude soroban-cli --profile test-wasms --target wasm32-unknown-unknown

test: build-test-wasms
	cargo test

check:
	cargo clippy --all-targets

watch:
	cargo watch --clear --watch-when-idle --shell '$(MAKE)'

fmt:
	cargo fmt --all

clean:
	cargo clean

publish:
	cargo workspaces publish --all --force '*' --from-git --yes
