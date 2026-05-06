FROM rust:latest

RUN rustup target add wasm32v1-none

RUN apt-get update && \
    apt-get install -y --no-install-recommends libudev1 libssl3 && \
    rm -rf /var/lib/apt/lists/*

ARG TARGETARCH
COPY stellar-${TARGETARCH}/stellar /usr/local/bin/stellar
RUN chmod +x /usr/local/bin/stellar

ENV STELLAR_CONFIG_HOME=/config
ENV STELLAR_DATA_HOME=/data

COPY entrypoint.sh /usr/local/bin/entrypoint.sh

WORKDIR /source

ENTRYPOINT ["/usr/local/bin/entrypoint.sh", "stellar"]
CMD []
