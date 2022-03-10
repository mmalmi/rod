FROM rust:latest

WORKDIR /usr/src/gun-rs

COPY Cargo.toml .
COPY Cargo.lock .
COPY assets assets
COPY src src

RUN cargo build --release

CMD ["./target/release/gundb", "start"]