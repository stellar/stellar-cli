all: check build test

export RUSTFLAGS=-Dwarnings -Dclippy::all -Dclippy::pedantic -Aclippy::match-same-arms

test:
	cargo test

build:
	cargo build

check:
	cargo clippy --all-targets

watch:
	cargo watch --clear --watch-when-idle --shell '$(MAKE)'
