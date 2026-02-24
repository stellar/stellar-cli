FROM rust:latest

RUN rustup target add wasm32v1-none

RUN apt-get update && \
    apt-get install -y --no-install-recommends dbus gnome-keyring libdbus-1-3 libudev1 libssl3 && \
    LATEST=$(curl -s https://api.github.com/repos/stellar/stellar-cli/releases/latest | grep '"tag_name"' | sed 's/.*"v\(.*\)".*/\1/') && \
    ARCH=$(dpkg --print-architecture) && \
    curl -fsSL "https://github.com/stellar/stellar-cli/releases/download/v${LATEST}/stellar-cli_${LATEST}_${ARCH}.deb" \
      -o /tmp/stellar-cli.deb && \
    dpkg -i /tmp/stellar-cli.deb && \
    rm -rf /var/lib/apt/lists/* /tmp/stellar-cli.deb

ENV STELLAR_CONFIG_HOME=/config
ENV STELLAR_DATA_HOME=/data

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

WORKDIR /source

ENTRYPOINT ["/usr/local/bin/entrypoint.sh", "stellar"]
CMD []
