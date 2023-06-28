FROM golang:1.20-bullseye as build
ARG RUST_TOOLCHAIN_VERSION=stable
ARG REPOSITORY_VERSION

WORKDIR /go/src/github.com/stellar/soroban-tools

ADD . ./

RUN git config --global --add safe.directory "/go/src/github.com/stellar/soroban-tools"

ENV CARGO_HOME=/rust/.cargo
ENV RUSTUP_HOME=/rust/.rust
ENV PATH="/usr/local/go/bin:$CARGO_HOME/bin:${PATH}"
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update
RUN apt-get install -y build-essential
RUN apt-get clean

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain $RUST_TOOLCHAIN_VERSION

RUN make REPOSITORY_VERSION=${REPOSITORY_VERSION} build-soroban-rpc
RUN mv soroban-rpc /bin/soroban-rpc

FROM ubuntu:20.04
ARG STELLAR_CORE_VERSION
ENV STELLAR_CORE_VERSION=${STELLAR_CORE_VERSION:-*}
ENV STELLAR_CORE_BINARY_PATH /usr/bin/stellar-core
ENV DEBIAN_FRONTEND=noninteractive

# ca-certificates are required to make tls connections
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl wget gnupg apt-utils
RUN wget -qO - https://apt.stellar.org/SDF.asc | APT_KEY_DONT_WARN_ON_DANGEROUS_USAGE=true apt-key add -
RUN echo "deb https://apt.stellar.org focal stable" >/etc/apt/sources.list.d/SDF.list
RUN echo "deb https://apt.stellar.org focal unstable" >/etc/apt/sources.list.d/SDF-unstable.list
RUN apt-get update && apt-get install -y stellar-core=${STELLAR_CORE_VERSION}
RUN apt-get clean

COPY --from=build /bin/soroban-rpc /app/
ENTRYPOINT ["/app/soroban-rpc"]
