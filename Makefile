all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic

install:
	cargo install --path .

test:
	cargo test

e2e-test:
	cargo test --features e2e_tests --test 'e2e*'

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

publish:
	cargo workspaces publish --all --force '*' --from-git --yes
