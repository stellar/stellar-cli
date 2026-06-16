FROM rust:latest

RUN rustup target add wasm32v1-none

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    git \
    libdbus-1-dev \
    libssl-dev \
    libudev-dev \
    pkg-config && \
    rm -rf /var/lib/apt/lists/*

ARG STELLAR_CLI_REV
RUN cargo install --locked \
        --git https://github.com/stellar/stellar-cli.git \
        --rev "${STELLAR_CLI_REV}" \
        stellar-cli

ENV STELLAR_CONFIG_HOME=/config
ENV STELLAR_DATA_HOME=/data
ENV STELLAR_NO_UPDATE_CHECK=1

COPY entrypoint.sh /usr/local/bin/entrypoint.sh

WORKDIR /source

ENTRYPOINT ["/usr/local/bin/entrypoint.sh", "stellar"]
CMD []
