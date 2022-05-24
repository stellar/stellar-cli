all: build test

test:
	cargo test

build:
	cargo check

watch:
	cargo watch --clear --watch-when-idle --shell '$(MAKE)'
