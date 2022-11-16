FROM golang:1.19.1 as build

ADD . /src/soroban-rpc
WORKDIR /src/soroban-rpc
RUN go build -o /bin/soroban-rpc ./exp/services/soroban-rpc


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
