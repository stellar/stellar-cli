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

ARG WASM_TARGET
RUN rustup target add ${WASM_TARGET}

ARG STELLAR_CLI_REPO
ARG STELLAR_CLI_REV
RUN cargo install \
    --locked \
    --git ${STELLAR_CLI_REPO} \
    --rev ${STELLAR_CLI_REV} \
    stellar-cli
