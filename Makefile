all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic

# set the GIT_REVISION unless it's already set by the caller.
export GIT_REVISION?=0.0.2

# update the Cargo.lock every time the Cargo.toml changes.
Cargo.lock: Cargo.toml
	cargo update

install: Cargo.lock
	cargo install --path .
	go install ./...

build: Cargo.lock
	cargo build
	go build ./...

build-test-wasms: Cargo.lock
	cargo build --package 'test_*' --profile test-wasms --target wasm32-unknown-unknown

test: build-test-wasms
	cargo test --workspace

e2e-test:
	cargo test --test 'e2e*' -- --ignored

check: Cargo.lock
	cargo clippy --all-targets

watch:
	cargo watch --clear --watch-when-idle --shell '$(MAKE)'

fmt:
	cargo fmt --all

clean:
	cargo clean
	go clean ./...

publish:
	cargo workspaces publish --all --force '*' --from-git --yes
