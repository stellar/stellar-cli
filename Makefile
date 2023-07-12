all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic

REPOSITORY_COMMIT_HASH := "$(shell git rev-parse HEAD)"
ifeq (${REPOSITORY_COMMIT_HASH},"")
	$(error failed to retrieve git head commit hash)
endif
# Want to treat empty assignment, `REPOSITORY_VERSION=` the same as absence or unset.
# By default make `?=` operator will treat empty assignment as a set value and will not use the default value.
# Both cases should fallback to default of getting the version from git tag.
ifeq ($(strip $(REPOSITORY_VERSION)),)
	override REPOSITORY_VERSION = "$(shell git describe --tags --always --abbrev=0 --match='v[0-9]*.[0-9]*.[0-9]*' 2> /dev/null | sed 's/^.//')"
endif  
REPOSITORY_BRANCH := "$(shell git rev-parse --abbrev-ref HEAD)"
BUILD_TIMESTAMP ?= $(shell date '+%Y-%m-%dT%H:%M:%S')
GOLDFLAGS :=	-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.Version=${REPOSITORY_VERSION}' \
				-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.CommitHash=${REPOSITORY_COMMIT_HASH}' \
				-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.BuildTimestamp=${BUILD_TIMESTAMP}' \
				-X 'github.com/stellar/soroban-tools/cmd/soroban-rpc/internal/config.Branch=${REPOSITORY_BRANCH}'


# The following works around incompatiblity beween the rust and the go linkers -
# the rust would generate an object file with min-version of 13.0 where-as the go
# compiler would generate a binary compatible with 12.3 and up. To align these
# we instruct the go comiler to produce binaries comparible with version 13.0.
# this is a mac-only limitation.
ifeq ($(shell uname -s),Darwin)
	MACOS_MIN_VER = -ldflags='-extldflags -mmacosx-version-min=13.0'
endif

# Always specify the build target so that libpreflight.a is always put into
# an architecture subdirectory (i.e. target/$(CARGO_BUILD_TARGET)/release-with-panic-unwind )
# Otherwise it will be much harder for Golang to find the library since
# it would need to distinguish when we are crosscompiling and when we are not
# (libpreflight.a is put at target/release-with-panic-unwind/ when not cross compiling)
CARGO_BUILD_TARGET ?= $(shell rustc -vV | sed -n 's|host: ||p')

# update the Cargo.lock every time the Cargo.toml changes.
Cargo.lock: Cargo.toml
	cargo update --workspace

install_rust: Cargo.lock
	cargo install --path ./cmd/soroban-cli --debug
	cargo install --path ./cmd/crates/soroban-test/tests/fixtures/hello --root ./target --debug --quiet

install: install_rust build-libpreflight
	go install -ldflags="${GOLDFLAGS}" ${MACOS_MIN_VER} ./...

build_rust: Cargo.lock
	cargo build

build_go: build-libpreflight
	go build -ldflags="${GOLDFLAGS}" ${MACOS_MIN_VER} ./...

# regenerate the example lib in `cmd/crates/soroban-spec-typsecript/fixtures/ts`
build-snapshot: typescript-bindings-fixtures

build: build_rust build_go

build-libpreflight: Cargo.lock
	cd cmd/soroban-rpc/lib/preflight && cargo build --target $(CARGO_BUILD_TARGET) --profile release-with-panic-unwind

build-test-wasms: Cargo.lock
	cargo build --package 'test_*' --profile test-wasms --target wasm32-unknown-unknown

build-test: build-test-wasms install_rust

test: build-test
	cargo test 

e2e-test:
	cargo test --test it -- --ignored

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

# the build-soroban-rpc build target is an optimized build target used by 
# https://github.com/stellar/pipelines/stellar-horizon/Jenkinsfile-soroban-rpc-package-builder
# as part of the package building.
build-soroban-rpc: build-libpreflight
	go build -ldflags="${GOLDFLAGS}" ${MACOS_MIN_VER} -o soroban-rpc -trimpath -v ./cmd/soroban-rpc

lint-changes:
	golangci-lint run ./... --new-from-rev $$(git rev-parse HEAD)

lint:
	golangci-lint run ./...

typescript-bindings-fixtures: build-test-wasms
	cargo run -- contract bindings typescript \
					--wasm ./target/wasm32-unknown-unknown/test-wasms/test_custom_types.wasm \
					--contract-name test_custom_types \
					--contract-id CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE \
					--network futurenet \
					--output-dir ./cmd/crates/soroban-spec-typescript/fixtures


# PHONY lists all the targets that aren't file names, so that make would skip the timestamp based check.
.PHONY: publish clean fmt watch check e2e-test test build-test-wasms install build build-soroban-rpc build-libpreflight lint lint-changes build-snapshot typescript-bindings-fixtures
