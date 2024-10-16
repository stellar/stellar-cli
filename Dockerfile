FROM rust:bookworm AS builder
WORKDIR /wd
COPY . .
RUN cargo install --locked --path cmd/stellar-cli --bin stellar --features opt

FROM gcr.io/distroless/cc-debian12:latest
COPY --from=builder /usr/local/cargo/bin/stellar /usr/local/bin/stellar
ENTRYPOINT ["stellar"]
