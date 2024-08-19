FROM rust:latest as builder
WORKDIR /wd
COPY . .
RUN cargo install --locked --path cmd/stellar-cli --bin stellar --features opt

FROM debian:latest
COPY --from=builder /usr/local/cargo/bin/stellar /usr/local/bin/stellar
ENTRYPOINT ["stellar"]
