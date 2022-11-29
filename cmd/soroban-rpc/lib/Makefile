
ifndef CARGO_BUILD_TARGET
export CARGO_BUILD_TARGET=$(shell rustc -vV | sed -n 's|host: ||p')
endif

.PHONY: all
all: lib/preflight/target/${CARGO_BUILD_TARGET}/release/libpreflight.a

lib/preflight/target/${CARGO_BUILD_TARGET}/release/libpreflight.a: preflight.h preflight/Cargo.toml preflight/src/lib.rs
	cd preflight && cargo build --release

.PHONY: clean
clean:
	rm -rf preflight/target
