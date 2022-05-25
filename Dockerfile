FROM rust:latest

WORKDIR /usr/src/rod

COPY Cargo.toml .
COPY Cargo.lock .
COPY assets assets
COPY src src

RUN cargo build --release

CMD ["./target/release/rod", "start"]
