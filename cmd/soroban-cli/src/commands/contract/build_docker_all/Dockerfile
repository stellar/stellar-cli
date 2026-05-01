# syntax=docker/dockerfile:1
#
# Build image used by `stellar contract build --backend docker-all`.
#
# Layered on top of a user-chosen rust base image so the rust toolchain stays
# pinnable to whatever the user trusts. The stellar-cli is installed inside
# at the same git revision the host stellar-cli was built from, so the entire
# build pipeline (cargo + meta injection + spec filtering + optimize) runs
# inside the container and the image captures everything except the source.

ARG BASE_IMAGE
FROM ${BASE_IMAGE}

# stellar-cli's default `additional-libs` feature bundles `wasm-opt`,
# `keyring`, and `stellar-ledger`. On Linux these pull in C deps:
#   - keyring → libdbus-sys      → libdbus-1-dev
#   - stellar-ledger → hidapi    → libudev-dev
#   - common → openssl-sys       → libssl-dev
# We don't use keyring or ledger inside this container — it only runs
# `stellar contract build` — but the binary still has to compile. Installing
# the dev headers is the simplest path; a future cleanup is to split
# `additional-libs` into separate features so docker-all can install with
# just `--features wasm-opt`.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        libdbus-1-dev \
        libudev-dev \
        libssl-dev \
        pkg-config && \
    rm -rf /var/lib/apt/lists/*

ARG WASM_TARGET
RUN rustup target add ${WASM_TARGET}

ARG STELLAR_CLI_REPO
ARG STELLAR_CLI_REV
RUN cargo install \
    --locked \
    --git ${STELLAR_CLI_REPO} \
    --rev ${STELLAR_CLI_REV} \
    stellar-cli
