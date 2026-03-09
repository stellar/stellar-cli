FROM rust:latest AS builder

ARG STELLAR_CLI_REF=main

RUN apt-get update && \
    apt-get install -y --no-install-recommends libdbus-1-dev libudev-dev pkg-config git && \
    rm -rf /var/lib/apt/lists/*

RUN git clone https://github.com/stellar/stellar-cli.git /tmp/stellar-cli && \
    cd /tmp/stellar-cli && \
    git fetch origin "${STELLAR_CLI_REF}" && \
    git checkout "${STELLAR_CLI_REF}" && \
    cargo install --locked --path cmd/stellar-cli && \
    rm -rf /tmp/stellar-cli

FROM rust:latest

RUN rustup target add wasm32v1-none

RUN apt-get update && \
    apt-get install -y --no-install-recommends dbus gnome-keyring libdbus-1-3 libudev1 libssl3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/stellar /usr/local/bin/stellar

ENV STELLAR_CONFIG_HOME=/config
ENV STELLAR_DATA_HOME=/data

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

WORKDIR /source

ENTRYPOINT ["/usr/local/bin/entrypoint.sh", "stellar"]
CMD []
