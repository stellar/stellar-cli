all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic

REPOSITORY_VERSION ?= "$(shell git describe --tags --always --abbrev=0 --match='v[0-9]*.[0-9]*.[0-9]*' 2> /dev/null | sed 's/^.//')"
REPOSITORY_COMMIT_HASH := "$(shell git rev-parse HEAD)"
REPOSITORY_BRANCH := "$(shell git rev-parse --abbrev-ref HEAD)"
BUILD_TIMESTAMP ?= $(shell date '+%Y-%m-%dT%H:%M:%S')
GOLDFLAGS :=	-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.Version=${REPOSITORY_VERSION}' \
				-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.CommitHash=${REPOSITORY_COMMIT_HASH}' \
				-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.BuildTimestamp=${BUILD_TIMESTAMP}' \
				-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.Branch=${REPOSITORY_BRANCH}'

# update the Cargo.lock every time the Cargo.toml changes.
Cargo.lock: Cargo.toml
	cargo update --workspace

install: Cargo.lock
	cargo install --path .
	go install -ldflags="${GOLDFLAGS}" ./...

build: Cargo.lock
	cargo build
	go build -ldflags="${GOLDFLAGS}" ./...

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
