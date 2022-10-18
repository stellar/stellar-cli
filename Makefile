all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic

install:
	cargo install --path .

test:
	cargo test

build:
	cargo build --workspace --exclude soroban-cli --profile test-wasms --target wasm32-unknown-unknown
	cargo build

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
