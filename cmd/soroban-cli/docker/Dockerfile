ARG DOCKERHUB_RUST_VERSION
ARG SOROBAN_CLI_CRATE_VERSION

FROM $DOCKERHUB_RUST_VERSION AS builder

WORKDIR /soroban-tools
COPY . .

run if [ ! -z "$SOROBAN_CLI_CRATE_VERSION" ]; then \
		cargo install \
		--config net.git-fetch-with-cli=true \
		--config build.jobs=6 -f --locked soroban-cli \
		--version "$SOROBAN_CLI_CRATE_VERSION"; \
	else \
		make install_rust; \
	fi
