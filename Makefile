all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic

REPOSITORY_COMMIT_HASH := "$(shell git rev-parse HEAD || echo NOREPO)"
ifeq (${REPOSITORY_COMMIT_HASH},"")
	$(error failed to retrieve git head commit hash)
endif
# Want to treat empty assignment, `REPOSITORY_VERSION=` the same as absence or unset.
# By default make `?=` operator will treat empty assignment as a set value and will not use the default value.
# Both cases should fallback to default of getting the version from git tag.
ifeq ($(strip $(REPOSITORY_VERSION)),)
	override REPOSITORY_VERSION = "$(shell ( git describe --tags --always --abbrev=0 --match='v[0-9]*.[0-9]*.[0-9]*' 2> /dev/null | sed 's/^.//' || echo NOREPO ) )"
endif  
REPOSITORY_BRANCH := "$(shell ( git rev-parse --abbrev-ref HEAD || echo NOREPO ) )"
BUILD_TIMESTAMP ?= $(shell date '+%Y-%m-%dT%H:%M:%S')

SOROBAN_PORT?=8000

# The following works around incompatibility between the rust and the go linkers -
# the rust would generate an object file with min-version of 13.0 where-as the go
# compiler would generate a binary compatible with 12.3 and up. To align these
# we instruct the go compiler to produce binaries comparible with version 13.0.
# this is a mac-only limitation.
ifeq ($(shell uname -s),Darwin)
	MACOS_MIN_VER = -ldflags='-extldflags -mmacosx-version-min=13.0'
endif

install_rust: install

install:
	cargo install --force --locked --path ./cmd/stellar-cli --debug
	cargo install --force --locked --path ./cmd/crates/soroban-test/tests/fixtures/hello --root ./target --debug --quiet
	cargo install --force --locked --path ./cmd/crates/soroban-test/tests/fixtures/bye --root ./target --debug --quiet

# regenerate the example lib in `cmd/crates/soroban-spec-typsecript/fixtures/ts`
build-snapshot: typescript-bindings-fixtures

build:
	cargo build

build-test-wasms:
	cargo build --package 'test_*' --profile test-wasms --target wasm32-unknown-unknown

build-test: build-test-wasms install

generate-full-help-doc:
	cargo run --bin doc-gen --features clap-markdown

test: build-test
	cargo test

e2e-test:
	cargo test --features it --test it -- integration

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

typescript-bindings-fixtures: build-test-wasms
	cargo run -- contract bindings typescript \
					--wasm ./target/wasm32-unknown-unknown/test-wasms/test_custom_types.wasm \
					--contract-id CBYMYMSDF6FBDNCFJCRC7KMO4REYFPOH2U4N7FXI3GJO6YXNCQ43CDSK \
					--network futurenet \
					--output-dir ./cmd/crates/soroban-spec-typescript/fixtures/test_custom_types \
					--overwrite


# PHONY lists all the targets that aren't file names, so that make would skip the timestamp based check.
.PHONY: publish clean fmt watch check e2e-test test build-test-wasms install build build-snapshot typescript-bindings-fixtures
