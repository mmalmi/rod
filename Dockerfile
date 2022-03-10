FROM rust:latest

WORKDIR /usr/src/gun-rs
COPY . .

RUN cargo build

CMD ["./target/debug/gundb", "start"]