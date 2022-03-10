FROM rust:latest

WORKDIR /usr/src/gun-rs
COPY . .

RUN cargo build

CMD ["/usr/src/gun-rs/target/debug/gundb", "start"]