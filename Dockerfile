ARG RUST_IMAGE=latest

FROM rust:${RUST_IMAGE} AS builder

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential cmake pkg-config git \
        libssl-dev libudev-dev libdbus-1-dev && \
    rm -rf /var/lib/apt/lists/*

ARG CLI_REPO=https://github.com/stellar/stellar-cli
ARG CLI_REF=main

WORKDIR /src
RUN git clone "${CLI_REPO}" . && \
    git checkout "${CLI_REF}"

RUN cargo build --package stellar-cli --release

FROM rust:${RUST_IMAGE}

RUN rustup target add wasm32v1-none

RUN apt-get update && \
    apt-get install -y --no-install-recommends libudev1 libssl3 libdbus-1-3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/stellar /usr/local/bin/stellar
RUN chmod +x /usr/local/bin/stellar

ENV STELLAR_CONFIG_HOME=/config
ENV STELLAR_DATA_HOME=/data

COPY entrypoint.sh /usr/local/bin/entrypoint.sh

WORKDIR /source

ENTRYPOINT ["/usr/local/bin/entrypoint.sh", "stellar"]
CMD []
